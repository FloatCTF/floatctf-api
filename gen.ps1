sea-orm-cli generate entity -o src/entity --with-serde both
cargo watch --watch src/ --ignore src/sql/ -x run