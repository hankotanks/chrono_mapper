fn main() -> Result<(), String> {
    pollster::block_on(app::Wrapper::run())
}
