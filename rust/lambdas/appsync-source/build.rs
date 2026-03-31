// build.rs runs at compile time BEFORE the crate is compiled.
// cargo:rerun-if-changed tells Cargo to re-run this script (and recompile the crate)
// only when the specified file changes. This is critical because make_operation! and
// make_types! read the GraphQL schema at compile time to generate code.
fn main() {
    // Tell Cargo to rerun this build script if the GraphQL schema changes
    // JRO comment: Does not seem to work very well though -_-'
    println!("cargo:rerun-if-changed=../../../graphql/schema.gql");
}
