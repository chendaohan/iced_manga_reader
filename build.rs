fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src/bin/protos")
        .compile(&["protos/manga.proto"], &["protos"])?;

    Ok(())
}
