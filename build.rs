use std::process::Command;

fn main() {
    Command::new("sh")
        .arg("-c")
        .arg("tailwindcss -i tailwind.css -o static/tailwind.css -m")
        .status()
        .expect("error running tailwind");
}
