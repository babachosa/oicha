use grid::{Grid, Order};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn grid_dimension_overflow_panics_before_mutation_in_both_orders() {
    for order in [Order::RowMajor, Order::ColumnMajor] {
        for grow_rows in [true, false] {
            for count in [usize::MAX / 2, usize::MAX] {
                let mut grid = Grid::from_vec_with_order(vec![1u8, 2, 3, 4], 2, order);
                let before = grid.clone();
                assert!(
                    catch_unwind(AssertUnwindSafe(|| {
                        if grow_rows {
                            grid.expand_rows(count);
                        } else {
                            grid.expand_cols(count);
                        }
                    }))
                    .is_err()
                );
                assert_eq!(grid, before);
                assert_eq!(grid.get(1, 1), Some(&4));
            }
        }
    }
}

#[test]
fn grid_valid_growth_preserves_values_in_both_orders() {
    for order in [Order::RowMajor, Order::ColumnMajor] {
        let mut grid = Grid::from_vec_with_order(vec![1u8, 2, 3, 4], 2, order);
        grid.expand_rows(2);
        grid.expand_cols(3);
        assert_eq!(grid.size(), (4, 5));
        assert_eq!(grid.get(1, 1), Some(&4));
        assert_eq!(grid.get(3, 4), Some(&0));
    }
}
