//! Tests for [`PageInfo`] accessors and position behaviour.

use rstest::rstest;

use crate::github::PageInfo;

#[derive(Debug, Clone, Copy)]
enum PageInfoPositionCase {
    First,
    Middle,
    Last,
}

#[rstest]
#[case::first(PageInfoPositionCase::First)]
#[case::middle(PageInfoPositionCase::Middle)]
#[case::last(PageInfoPositionCase::Last)]
fn position_behaviour(#[case] case: PageInfoPositionCase) {
    let (current_page, has_next, has_prev) = match case {
        PageInfoPositionCase::First => (1, true, false),
        PageInfoPositionCase::Middle => (2, true, true),
        PageInfoPositionCase::Last => (5, false, true),
    };

    let info = PageInfo::builder(current_page, 50)
        .total_pages(Some(5))
        .has_next(has_next)
        .has_prev(has_prev)
        .build();

    assert_eq!(info.has_next(), has_next, "unexpected has_next");
    assert_eq!(info.has_prev(), has_prev, "unexpected has_prev");

    let is_first_page = matches!(case, PageInfoPositionCase::First);
    let is_last_page = matches!(case, PageInfoPositionCase::Last);
    assert_eq!(
        info.is_first_page(),
        is_first_page,
        "unexpected is_first_page"
    );
    assert_eq!(info.is_last_page(), is_last_page, "unexpected is_last_page");
}

#[rstest]
fn accessors() {
    let info = PageInfo::builder(3, 25)
        .total_pages(Some(10))
        .has_next(true)
        .has_prev(true)
        .build();
    assert_eq!(info.current_page(), 3, "current page mismatch");
    assert_eq!(info.per_page(), 25, "per page mismatch");
    assert_eq!(info.total_pages(), Some(10), "total pages mismatch");
}
