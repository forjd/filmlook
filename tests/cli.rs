use std::{fs, fs::create_dir_all, process::Command};

use image::{GenericImageView, ImageBuffer, Rgb};
use tempfile::tempdir;

#[test]
fn processes_single_image() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.jpg");
    write_sample(&input);

    let status = Command::new(env!("CARGO_BIN_EXE_filmlook"))
        .arg(&input)
        .arg(&output)
        .args([
            "--recipe",
            "portra-400-35mm",
            "--grain",
            "0.45",
            "--halation",
            "0.2",
        ])
        .status()
        .expect("run filmlook");

    assert!(status.success());
    assert_eq!(
        image::open(&output).expect("decode output").dimensions(),
        (10, 8)
    );
}

#[test]
fn processes_single_image_with_recipe_file() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.jpg");
    let recipe = dir.path().join("shared-recipe.json");
    write_sample(&input);
    fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/recipes/builtin/portra-400-35mm.json"
        ),
        &recipe,
    )
    .expect("copy recipe");

    let status = Command::new(env!("CARGO_BIN_EXE_filmlook"))
        .arg(&input)
        .arg(&output)
        .args([
            "--recipe",
            recipe.to_str().expect("recipe path"),
            "--grain",
            "0.2",
        ])
        .status()
        .expect("run filmlook");

    assert!(status.success());
    assert_eq!(
        image::open(&output).expect("decode output").dimensions(),
        (10, 8)
    );
}

#[test]
fn lists_and_validates_recipes() {
    let list_output = Command::new(env!("CARGO_BIN_EXE_filmlook"))
        .arg("--list-recipes")
        .output()
        .expect("list recipes");

    assert!(list_output.status.success());
    assert!(String::from_utf8_lossy(&list_output.stdout).contains("portra-400-35mm"));
    assert!(String::from_utf8_lossy(&list_output.stdout).contains("kodak-gold-200"));
    assert!(String::from_utf8_lossy(&list_output.stdout).contains("ilford-hp5-plus-400"));

    let validate_output = Command::new(env!("CARGO_BIN_EXE_filmlook"))
        .arg("--validate-recipe")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/recipes/builtin/kodak-gold-200.json"
        ))
        .output()
        .expect("validate recipe");

    assert!(validate_output.status.success());
}

#[test]
fn rejects_removed_preset_flag() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.jpg");
    write_sample(&input);

    let run_output = Command::new(env!("CARGO_BIN_EXE_filmlook"))
        .arg(&input)
        .arg(&output)
        .args(["--preset", "portra-400-35mm"])
        .output()
        .expect("run filmlook");

    assert!(!run_output.status.success());
    assert!(!output.exists());
}

#[test]
fn processes_batch_folder_recursively() {
    let dir = tempdir().expect("tempdir");
    let input_dir = dir.path().join("input");
    let nested_dir = input_dir.join("nested");
    let output_dir = dir.path().join("output");
    create_dir_all(&nested_dir).expect("create nested input");
    write_sample(&input_dir.join("a.png"));
    write_sample(&nested_dir.join("b.jpg"));

    let status = Command::new(env!("CARGO_BIN_EXE_filmlook"))
        .arg(&input_dir)
        .arg(&output_dir)
        .args([
            "--recursive",
            "--recipe",
            "ilford-hp5-plus-400",
            "--seed",
            "7",
        ])
        .status()
        .expect("run filmlook");

    assert!(status.success());
    assert_eq!(
        image::open(output_dir.join("a.png"))
            .expect("decode top-level output")
            .dimensions(),
        (10, 8)
    );
    assert_eq!(
        image::open(output_dir.join("nested/b.jpg"))
            .expect("decode nested output")
            .dimensions(),
        (10, 8)
    );
}

fn write_sample(path: &std::path::Path) {
    let image = ImageBuffer::from_fn(10, 8, |x, y| {
        Rgb([
            (x * 18 + 30) as u8,
            (y * 24 + 20) as u8,
            ((x + y) * 14 + 35) as u8,
        ])
    });
    image.save(path).expect("save sample");
}
