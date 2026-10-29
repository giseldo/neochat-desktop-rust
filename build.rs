use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/neochat.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    println!("cargo:rerun-if-changed=assets/neochat.png");
    println!("cargo:rerun-if-changed=scripts/gen-icon.ps1");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        println!("cargo:warning=icone e manifesto do executavel so sao embutidos no alvo MSVC");
        return;
    }

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let icon = manifest_dir.join("assets").join("neochat.ico");
    let manifest = manifest_dir.join("assets").join("app.manifest");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

    if !icon.is_file() {
        println!("cargo:warning=assets/neochat.ico nao encontrado; rode scripts/gen-icon.ps1");
        return;
    }

    let rc_file = out_dir.join("neochat.rc");
    let res_file = out_dir.join("neochat.res");
    let contents = format!(
        "1 ICON \"{}\"\n1 24 \"{}\"\n",
        escape(&icon),
        escape(&manifest)
    );
    if let Err(err) = std::fs::write(&rc_file, contents) {
        println!("cargo:warning=falha ao escrever o arquivo .rc: {err}");
        return;
    }

    let Some(rc_exe) = find_rc() else {
        println!("cargo:warning=rc.exe nao encontrado; o executavel ficara sem icone e manifesto");
        return;
    };

    match Command::new(&rc_exe)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res_file)
        .arg(&rc_file)
        .output()
    {
        Ok(output) if output.status.success() => {
            println!("cargo:rustc-link-arg={}", res_file.display());
        }
        Ok(output) => {
            println!(
                "cargo:warning=rc.exe falhou ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Err(err) => println!("cargo:warning=falha ao executar rc.exe: {err}"),
    }
}

fn escape(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

fn find_rc() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("RC") {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let kits = std::env::var("ProgramFiles(x86)")
        .or_else(|_| std::env::var("ProgramFiles"))
        .ok()?;
    let bin = PathBuf::from(kits)
        .join("Windows Kits")
        .join("10")
        .join("bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&bin)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .collect();
    versions.sort();
    versions.reverse();

    for version in versions {
        for arch in ["x64", "x86"] {
            let candidate = version.join(arch).join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
