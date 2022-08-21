pub struct Checksum {
}

impl Checksum {
    pub fn new() -> Self {
        Self{}
    }

    pub fn run(&self, elements: &[&dyn crate::checksum::Checksum]) -> u16 {
        let mut csum: u16 = 0;
        for e in elements {
            csum += e.csum();
        }
        csum
    }

}
