//! Install engine. System operations (ARP registry, shortcuts, PATH) go through
//! `reg.exe` / PowerShell subprocesses — reliable, no `unsafe`, and the process
//! already runs elevated (requireAdministrator manifest). File extraction uses
//! the embedded payload zip.
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone)]
pub struct Opts {
    pub dir: PathBuf,
    pub start_menu: bool,
    pub desktop: bool,
    pub add_path: bool,
}

/// One line in the live provisioning log.
#[derive(Clone)]
pub struct Step {
    pub label: String,
    pub frac: f32,
}

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let mut c = Command::new(cmd);
    c.args(args);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    match c.status() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("{cmd} exited with {s}")),
        Err(e) => Err(format!("{cmd} failed to start: {e}")),
    }
}

fn extract_zip(bytes: &[u8], dest: &Path) -> Result<u64, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("payload: {e}"))?;
    let mut written = 0u64;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| format!("payload entry: {e}"))?;
        let Some(rel) = f.enclosed_name() else { continue };
        let out = dest.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| format!("mkdir {out:?}: {e}"))?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("mkdir {p:?}: {e}"))?;
            }
            let mut o = std::fs::File::create(&out).map_err(|e| format!("create {out:?}: {e}"))?;
            written += std::io::copy(&mut f, &mut o).map_err(|e| format!("write {out:?}: {e}"))?;
        }
    }
    Ok(written)
}

fn arp_key() -> String {
    format!(
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        config::APP_NAME
    )
}

fn write_arp(dir: &Path, app_exe: &Path, uninst: &Path, size_kb: u32) -> Result<(), String> {
    let key = arp_key();
    let dir_s = dir.display().to_string();
    let icon_s = app_exe.display().to_string();
    let uninst_s = format!("\"{}\" --uninstall", uninst.display());
    let size_s = size_kb.to_string();
    let strs: [(&str, &str); 8] = [
        ("DisplayName", config::APP_NAME),
        ("DisplayVersion", config::VERSION),
        ("Publisher", config::PUBLISHER),
        ("InstallLocation", &dir_s),
        ("DisplayIcon", &icon_s),
        ("UninstallString", &uninst_s),
        ("QuietUninstallString", &uninst_s),
        ("URLInfoAbout", config::HOMEPAGE),
    ];
    for (n, val) in strs {
        run("reg", &["add", &key, "/v", n, "/t", "REG_SZ", "/d", val, "/f"])?;
    }
    for (n, val) in [("NoModify", "1"), ("NoRepair", "1"), ("EstimatedSize", &size_s)] {
        run("reg", &["add", &key, "/v", n, "/t", "REG_DWORD", "/d", val, "/f"])?;
    }
    Ok(())
}

fn ps(script: &str) -> Result<(), String> {
    run(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script],
    )
}

fn make_shortcut(lnk: &Path, target: &Path, workdir: &Path) -> Result<(), String> {
    if let Some(p) = lnk.parent() {
        std::fs::create_dir_all(p)
            .map_err(|e| format!("shortcut dir {:?}: {e}", lnk.parent().unwrap()))?;
    }
    // `$ErrorActionPreference='Stop'` + try/catch/exit-1 is load-bearing: a
    // `WScript.Shell` COM failure (e.g. `Save()` denied) is a NON-terminating
    // error, so a bare `-Command` script would still exit 0 and the installer
    // would silently report success with no shortcut on disk. The catch turns
    // any failure into a non-zero exit `ps()` surfaces; the post-Save existence
    // check below is the second, language-agnostic guard.
    let s = format!(
        "$ErrorActionPreference='Stop'; try {{ \
         $w=New-Object -ComObject WScript.Shell; $s=$w.CreateShortcut('{}'); \
         $s.TargetPath='{}'; $s.WorkingDirectory='{}'; $s.IconLocation='{},0'; \
         $s.Description='{}'; $s.Save() }} \
         catch {{ Write-Error $_; exit 1 }}",
        lnk.display(),
        target.display(),
        workdir.display(),
        target.display(),
        config::TAGLINE.replace('\'', " "),
    );
    ps(&s)?;
    // Verify the .lnk actually materialized — catches any silent COM failure
    // that still slipped an exit-0 past `ps()`.
    if !lnk.exists() {
        return Err(format!(
            "shortcut was not created at {} (the shell reported success but no .lnk exists)",
            lnk.display()
        ));
    }
    Ok(())
}

fn start_menu_dir() -> PathBuf {
    let pd = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    PathBuf::from(pd)
        .join(r"Microsoft\Windows\Start Menu\Programs")
        .join(config::VENDOR)
}

fn public_desktop() -> PathBuf {
    let pub_dir = std::env::var("PUBLIC").unwrap_or_else(|_| r"C:\Users\Public".into());
    PathBuf::from(pub_dir).join("Desktop")
}

fn add_to_path(dir: &Path) -> Result<(), String> {
    // Machine PATH; SetEnvironmentVariable broadcasts WM_SETTINGCHANGE for us.
    let d = dir.display().to_string();
    let script = format!(
        "$d='{d}'; $p=[Environment]::GetEnvironmentVariable('Path','Machine'); \
         if($p -notlike ('*'+$d+'*')){{ \
           $n=if([string]::IsNullOrEmpty($p)){{$d}}else{{$p.TrimEnd(';')+';'+$d}}; \
           [Environment]::SetEnvironmentVariable('Path',$n,'Machine') }}"
    );
    ps(&script)
}

/// Run the full install. `progress` receives (fraction, log-line) updates.
pub fn install(opts: &Opts, payload: &[u8], progress: &dyn Fn(Step)) -> Result<(), String> {
    let log = |frac: f32, label: &str| progress(Step { label: label.to_string(), frac });

    log(0.04, "designate partition");
    std::fs::create_dir_all(&opts.dir).map_err(|e| format!("create {:?}: {e}", opts.dir))?;

    log(0.12, "provision payload");
    let bytes = extract_zip(payload, &opts.dir)?;
    let size_kb = (bytes / 1024).max(1) as u32;

    log(0.58, "stage uninstaller");
    let me = std::env::current_exe().map_err(|e| format!("self path: {e}"))?;
    let uninst = opts.dir.join("uninstall.exe");
    std::fs::copy(&me, &uninst).map_err(|e| format!("stage uninstaller: {e}"))?;

    let app_exe = opts.dir.join(config::APP_BIN);
    log(0.68, "register node (ARP)");
    write_arp(&opts.dir, &app_exe, &uninst, size_kb)?;

    if opts.start_menu {
        log(0.80, "link start menu");
        let lnk = start_menu_dir().join(format!("{}.lnk", config::APP_NAME));
        make_shortcut(&lnk, &app_exe, &opts.dir)?;
    }
    if opts.desktop {
        log(0.88, "link desktop");
        let lnk = public_desktop().join(format!("{}.lnk", config::APP_NAME));
        make_shortcut(&lnk, &app_exe, &opts.dir)?;
    }
    if opts.add_path {
        log(0.94, "wire PATH");
        add_to_path(&opts.dir)?;
    }
    log(1.0, "node online");
    Ok(())
}

/// Path to the installed main binary for the "launch now" finish action.
pub fn installed_binary(dir: &Path) -> PathBuf {
    dir.join(config::APP_BIN)
}

/// Uninstall: remove shortcuts, ARP key, PATH entry, then schedule directory
/// removal via a detached cmd (a running exe can't delete its own folder).
pub fn uninstall() -> Result<(), String> {
    let me = std::env::current_exe().map_err(|e| format!("self path: {e}"))?;
    let dir = me.parent().map(Path::to_path_buf).unwrap_or_default();

    let _ = run("reg", &["delete", &arp_key(), "/f"]);
    let sm = start_menu_dir().join(format!("{}.lnk", config::APP_NAME));
    let _ = std::fs::remove_file(&sm);
    let _ = std::fs::remove_dir(start_menu_dir());
    let _ = std::fs::remove_file(public_desktop().join(format!("{}.lnk", config::APP_NAME)));

    // remove our install dir from machine PATH
    let d = dir.display().to_string();
    let _ = ps(&format!(
        "$d='{d}'; $p=[Environment]::GetEnvironmentVariable('Path','Machine'); \
         if($p){{ $n=($p -split ';' | Where-Object {{$_ -and ($_ -ne $d)}}) -join ';'; \
         [Environment]::SetEnvironmentVariable('Path',$n,'Machine') }}"
    ));

    // detached self-deleting cleanup of the install directory
    if !dir.as_os_str().is_empty() {
        let mut c = Command::new("cmd");
        c.args([
            "/C",
            &format!(
                "ping 127.0.0.1 -n 2 >nul & rmdir /s /q \"{}\"",
                dir.display()
            ),
        ]);
        #[cfg(windows)]
        c.creation_flags(CREATE_NO_WINDOW | 0x0000_0008 /*DETACHED_PROCESS*/);
        let _ = c.spawn();
    }
    Ok(())
}
