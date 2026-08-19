// Copyright 2026 Alexandre Mahdhaoui
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: &'static [u8],
}

impl Image {
    pub fn is_intact(&self) -> bool {
        self.rgba.len() == (self.width as usize) * (self.height as usize) * 4
    }
}

pub const ICON: Image = Image {
    width: 256,
    height: 256,
    rgba: include_bytes!("../../assets/icon-256.rgba"),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_holds_exactly_the_pixels_its_size_claims() {
        assert!(ICON.is_intact());
    }
}
