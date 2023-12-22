use std::process::Command;

fn main() {
    println!("cargo:rustc-rerun-if-changed=.git/HEAD");
    let version = if let Some(version) = {
        if let Ok(command_output) = Command::new("git")
            .args(["describe", "--always", "--tags", "--dirty"])
            .output()
        {
            if command_output.status.success() {
                Some(String::from_utf8(command_output.stdout).unwrap())
            } else {
                None
            }
        } else {
            None
        }
    } {
        version
    } else {
        env!("CARGO_PKG_VERSION").to_string()
    };
    println!("cargo:rustc-env=VERSION={}", version);
}
