use sea_orm::{Condition, EntityTrait, QueryFilter, Select};

pub struct FilterMapping {
    pub key: &'static str,
    pub column: Box<dyn Fn(&str) -> Condition>,
}

pub fn apply_filters<E: EntityTrait>(
    mut stmt: Select<E>,
    filter: Option<String>,
    mappings: &[FilterMapping],
) -> Select<E> {
    if let Some(f) = filter {
        let tokens: Vec<&str> = f.split_whitespace().collect();
        let mut i = 0;
        let mut or_conditions: Vec<Condition> = vec![];
        let mut and_conditions: Vec<Condition> = vec![];

        while i < tokens.len() {
            let token = tokens[i];

            match token {
                "&" => {
                    i += 1;
                    continue;
                }
                "|" => {
                    if !and_conditions.is_empty() {
                        // fold 累加多个 Condition
                        let combined = and_conditions
                            .drain(..)
                            .fold(Condition::all(), |c, cond| c.add(cond));
                        or_conditions.push(combined);
                    }
                    i += 1;
                }
                _ => {
                    if let Some(pos) = token.find(':') {
                        let key = &token[..pos];
                        let mut value = token[pos + 1..].to_string();

                        // 拼接后续 token，直到遇到逻辑符号
                        i += 1;
                        while i < tokens.len() && tokens[i] != "&" && tokens[i] != "|" {
                            value.push(' ');
                            value.push_str(tokens[i]);
                            i += 1;
                        }

                        if let Some(m) = mappings.iter().find(|m| m.key == key) {
                            and_conditions.push((m.column)(&value));
                        }
                    } else {
                        i += 1;
                    }
                }
            }
        }

        // 剩余 AND 条件加入 OR 条件
        if !and_conditions.is_empty() {
            let combined = and_conditions
                .drain(..)
                .fold(Condition::all(), |c, cond| c.add(cond));
            or_conditions.push(combined);
        }

        // 最终 OR 条件累加到 stmt
        if !or_conditions.is_empty() {
            let combined = or_conditions
                .into_iter()
                .fold(Condition::any(), |c, cond| c.add(cond));
            stmt = stmt.filter(combined);
        }
    }

    stmt
}
