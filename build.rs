use std::process::Command;

fn main() {
    let tailwind_cmd = "tailwindcss -i tailwind.css -o static/tailwind.css -m";

    Command::new("sh")
        .arg("-c")
        .arg(tailwind_cmd)
        .status()
        .expect("error running tailwind");

    println!("cargo:rerun-if-changed=tailwind.config.js");
    println!("cargo:rerun-if-changed=tailwind.css");
}
