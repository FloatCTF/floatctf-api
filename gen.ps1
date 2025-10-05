sea-orm-cli generate entity -o src/entity --with-serde both --enum-extra-attributes 'serde(rename_all = "snake_case")'
cargo watch --watch src/ --ignore src/sql/ -x run