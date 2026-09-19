fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .btree_map(["."])
        .compile_protos(&["proto/underpass/made/v1/made.proto"], &["proto"])?;
    Ok(())
}
