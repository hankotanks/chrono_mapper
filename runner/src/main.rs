fn main() -> Result<(), String> {
    pollster::block_on(lib::Wrapper::run())
}
