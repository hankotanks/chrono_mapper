fn main() -> Result<(), String> {
    backend_macros::run!(lib::Wrapper)
}
