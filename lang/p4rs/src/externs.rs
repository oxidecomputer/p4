pub struct Checksum {
}

impl Checksum {
    pub fn new() -> Self {
        Self{}
    }

    pub fn run(&self, elements: &[&dyn crate::checksum::Checksum]) -> u16 {
        todo!();
    }

}
