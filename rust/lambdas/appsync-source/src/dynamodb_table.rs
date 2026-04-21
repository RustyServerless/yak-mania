use dynamodb_facade::{
    StringAttribute, attribute_definitions, index_definitions, table_definitions,
};

attribute_definitions! {
    /// The partition key of the MonoTable
    PK {
        "PK": StringAttribute
    }
    /// The sort key of the MonoTable
    SK {
        "SK": StringAttribute
    }
    /// The "type" of the item going into the MonoTable
    ItemType {
        "_TYPE": StringAttribute
    }
}

table_definitions! {
    /// The definition of our monotable design
    MonoTable {
        type PartitionKey = PK;
        type SortKey = SK;
        fn table_name() -> String {
            let table_name = std::env::var("BACKEND_TABLE_NAME")
                .expect("Mandatory environment variable `BACKEND_TABLE_NAME` is not set");
            tracing::debug!("BACKEND_TABLE_NAME={table_name}");
            table_name
        }
    }
}

index_definitions! {
    /// The definition of the iType index
    #[table = MonoTable]
    TypeIndex {
        type PartitionKey = ItemType;
        fn index_name() -> String {
            "iType".to_owned()
        }
    }
}
