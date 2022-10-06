use page_turner::PageQuery;

#[derive(Debug, Default, Clone, PageQuery)]
struct QueryWithKeyAttribute {
    field1: usize,
    field2: usize,
    #[page_key]
    field3: usize,
}

fn main() {
    let mut query2 = QueryWithKeyAttribute::default();
    query2.set_page_key(32);
    assert_eq!(query2.field3, 32);
}
