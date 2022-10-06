use page_turner::PageQuery;

#[derive(Debug, Clone, PageQuery)]
struct WithoutKey {
    field1: String,
    field2: String,
    field3: String,
}

#[derive(Debug, Clone, PageQuery)]
struct ZeroStruct;

#[derive(Debug, Clone, PageQuery)]
struct TupleStruct(i32);

#[derive(Debug, Clone, PageQuery)]
enum NotAStruct {
    #[page_key]
    Variant1(String),
    Variant2(String),
}

fn main() {}
