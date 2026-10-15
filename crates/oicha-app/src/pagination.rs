//! Display pagination only. Page boundaries never insert manuscript characters.
pub const MARGIN: f32 = 76.;
pub const FIRST_TEXT_TOP: f32 = MARGIN;
pub const PAGE_GAP: f32 = 24.;

#[derive(Clone, Copy, Debug)]
pub struct PageGeometry {
    pub width: f32,
    pub height: f32,
}

impl PageGeometry {
    pub fn new(width: f32) -> Self {
        Self {
            width,
            height: (width * 297. / 210.).round(),
        }
    }
    pub fn stride(self) -> f32 {
        self.height + PAGE_GAP
    }
    pub fn top(self, page: usize) -> f32 {
        self.stride() * page as f32
    }
    pub fn text_bottom(self, page: usize) -> f32 {
        self.top(page) + self.height - MARGIN
    }
    pub fn total_height(self, pages: usize) -> f32 {
        self.top(pages.saturating_sub(1)) + self.height
    }
}

pub struct Paginator {
    pub geometry: PageGeometry,
    page: usize,
    next_y: f32,
    row_height: f32,
}

impl Paginator {
    pub fn new(geometry: PageGeometry, row_height: f32) -> Self {
        Self {
            geometry,
            page: 0,
            next_y: FIRST_TEXT_TOP,
            row_height,
        }
    }
    pub fn place_row(&mut self) -> f32 {
        if self.next_y + self.row_height > self.geometry.text_bottom(self.page) + 0.01 {
            self.page += 1;
            self.next_y = self.geometry.top(self.page) + MARGIN;
        }
        let y = self.next_y;
        self.next_y += self.row_height;
        y
    }
    pub fn pages(&self) -> usize {
        self.page + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheets_stay_fixed_until_a_row_requires_the_next_sheet() {
        let geometry = PageGeometry::new(760.);
        let mut pager = Paginator::new(geometry, 37.);
        assert!((geometry.height / geometry.width - 297. / 210.).abs() < 0.001);
        let first_height = geometry.total_height(pager.pages());
        let mut previous = pager.place_row();
        assert_eq!(previous, FIRST_TEXT_TOP);
        loop {
            let y = pager.place_row();
            if pager.pages() == 2 {
                assert_eq!(y, geometry.top(1) + MARGIN);
                assert!(y - previous > 37.);
                assert_eq!(geometry.total_height(2) - first_height, geometry.stride());
                break;
            }
            assert_eq!(geometry.total_height(pager.pages()), first_height);
            assert_eq!(y - previous, 37.);
            previous = y;
        }
    }

    #[test]
    fn rows_fit_within_margins_at_supported_widths_and_font_sizes() {
        for width in [484., 580., 760.] {
            for row_height in [22.4, 37., 51.8, 70.4] {
                let geometry = PageGeometry::new(width);
                let mut pager = Paginator::new(geometry, row_height);
                for _ in 0..1000 {
                    let y = pager.place_row();
                    let page = pager.pages() - 1;
                    assert!(y >= geometry.top(page) + MARGIN);
                    assert!(y + row_height <= geometry.text_bottom(page) + 0.01);
                }
            }
        }
    }
}
