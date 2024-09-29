use fallible_iterator::FallibleIterator;
use sqlite3_parser::lexer::sql::Parser;
use sqlite3_parser::ast::{Cmd, FromClause, OneSelect, Stmt, Select, SelectBody, SelectTable, QualifiedName};
use std::collections::HashSet;
use std::str;


pub fn parse_sql_command_for_table_names(sql_query: &str) -> HashSet<std::string::String> {
    let mut parser = Parser::new(sql_query.as_bytes());
    let cmd = parser.next();
    if let Ok(Some(cmd)) = cmd {
        match cmd {
            Cmd::Stmt(Stmt::Select(select)) => {
                let qualified_table_names = extract_table_names(&select);
                let table_names = extract_table_name_strings(&qualified_table_names);
                return table_names;
            },
            Cmd::Explain(_) | Cmd::ExplainQueryPlan(_) => todo!(),
            _ => todo!()
        }
    };
    HashSet::new()
}

fn extract_table_name_strings(qualified_names: &[QualifiedName]) -> HashSet<String> {
   qualified_names
       .iter()
       .map(|qn| {
           if let Some(db_name) = &qn.db_name {
               format!("{}\x1F{}", db_name.0, qn.name.0)
           } else {
               qn.name.0.clone()
           }
       })
       .collect()
}

fn extract_table_names(select: &Select) -> Vec<QualifiedName> {
    let mut table_names = Vec::new();
    extract_table_names_from_select(select, &mut table_names);
    table_names
}

fn extract_table_names_from_select(select: &Select, table_names: &mut Vec<QualifiedName>) {
    extract_table_names_from_select_body(&select.body, table_names);
}

fn extract_table_names_from_select_body(body: &SelectBody, table_names: &mut Vec<QualifiedName>) {
    extract_table_names_from_one_select(&body.select, table_names);
    if let Some(compounds) = &body.compounds {
        for compound in compounds {
            extract_table_names_from_one_select(&compound.select, table_names);
        }
    }
}

fn extract_table_names_from_one_select(one_select: &OneSelect, table_names: &mut Vec<QualifiedName>) {
    match one_select {
        OneSelect::Select { from, .. } => {
            if let Some(from_clause) = from {
                extract_table_names_from_from_clause(from_clause, table_names);
            }
        },
	OneSelect::Values(_) => {},
    }
}

fn extract_table_names_from_from_clause(from_clause: &FromClause, table_names:&mut Vec<QualifiedName>) {
    if let Some(select_table) = &from_clause.select {
        extract_table_names_from_select_table(select_table, table_names);
    }
    if let Some(joins) = &from_clause.joins {
        for join in joins {
            extract_table_names_from_select_table(&join.table, table_names);
        }
    }
}

fn extract_table_names_from_select_table(select_table: &SelectTable, table_names:&mut Vec<QualifiedName>) {
    match select_table {
        SelectTable::Table(qualified_name, _, _) => {
            add_unique_qualified_name(table_names, qualified_name);
        },
        SelectTable::TableCall(qualified_name, _, _,) => {
            add_unique_qualified_name(table_names, qualified_name);
        },
        SelectTable::Select(select, _) => {
            extract_table_names_from_select(select, table_names);
        },
        SelectTable::Sub(from_clause, _) => {
            extract_table_names_from_from_clause(from_clause, table_names);
        },
    }
}

fn add_unique_qualified_name(table_names: &mut Vec<QualifiedName>, new_name: &QualifiedName) {
    if !table_names.iter().any(|name| name == new_name) {
        table_names.push(new_name.clone());
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let message = "<sql_query>";
    let query = std::env::args().nth(1)
        .expect(format!(r#"Missing the sql query. Usage: rust_sql_parser "{}""#, message).as_str());
    let table_name_strings = parse_sql_command_for_table_names(&query); 
    let table_names_joined = table_name_strings.into_iter().collect::<Vec<_>>().join(",");
    println!("{}", table_names_joined);
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_simple_sql() {
        let sql_query = "SELECT *\nFROM bananas\nWHERE color = 'red'";
        let mut expected = HashSet::new();
        expected.insert("bananas".to_string());
        assert_eq!(parse_sql_command_for_table_names(sql_query), expected);
    }

    #[test]
    fn test_sql_join() {
        let sql_query = "Select m.title, r.id\n FROM Movies m\n INNER JOIN (\nSELECT rs.movie_id\n FROM Rooms r2 \n WHERE r2.seaats >= 50 \n ) AS r \n ON m.id = r.movide_id AND m.title != 'Batman';";
        let mut expected = HashSet::new();
        expected.insert("Movies".to_string());
        expected.insert("Rooms".to_string());    
        assert_eq!(parse_sql_command_for_table_names(sql_query), expected); 
    }
    
    #[test]
    fn test_sql_union() {
        let sql_query = "SELECT *\nFROM a\nUNION\nSELECT *\nFROM b";
        let mut expected = HashSet::new();
        expected.insert("a".to_string());
        expected.insert("b".to_string());
	assert_eq!(parse_sql_command_for_table_names(sql_query), expected);
    }

    #[test]
    fn test_sql_sub_query() {
        let sql_query = "SELECT a.color\nFROM (\nSELECT b.color\nFROM bananas b\n) z JOIN apples a\nON a.color = b.color";
	let mut expected = HashSet::new();
        expected.insert("apples".to_string());
        expected.insert("bananas".to_string());
        assert_eq!(parse_sql_command_for_table_names(sql_query), expected);
    }

    #[test]
    fn test_sql_backticks() {
        let sql_query = "SELECT\n  *\nFROM\n  `hats` h\nWHERE\n  h.color == 'red'\nGROUP BY\n  h.color, h.material\nHAVING\n  COUNT(h.quantity) >= 200\nORDER BY\n  h.color DESC\nLIMIT\n  20\nOFFSET\n  10";
        let mut expected = HashSet::new();
        expected.insert("`hats`".to_string());
        assert_eq!(parse_sql_command_for_table_names(sql_query), expected);
    }

    #[test]
    fn test_sql_db_name() {
        let sql_query = "SELECT *\nFROM apples.bananas\nWHERE color = 'red'";
        let mut expected = HashSet::new();
        expected.insert(format!("{}\x1F{}", "apples", "bananas").to_string());
        assert_eq!(parse_sql_command_for_table_names(sql_query), expected);
    }
}
