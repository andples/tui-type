//! Settings files that keep what ttyp doesn't know. `config.toml` and the
//! profiles are shared between machines (dotfiles, account sync), so a file
//! can name settings from a newer ttyp, or carry a typo and comments. Reading
//! lists the keys the type has no field for (`parse_with_unknown`); saving
//! edits the file in place (`merge_toml`), so those keys, their values and
//! every comment survive.

use serde::Serialize;
use serde::de::DeserializeOwned;
use toml_edit::{DocumentMut, Item, TableLike, Value};

/// Parse `text` as `T`, and list the keys `T` ignored, as dotted paths
/// (`fontsize`, `results.bogus`).
pub fn parse_with_unknown<T: DeserializeOwned>(
    text: &str,
) -> Result<(T, Vec<String>), toml::de::Error> {
    let mut unknown = Vec::new();
    let de = toml::Deserializer::parse(text)?;
    let value = serde_ignored::deserialize(de, |path| {
        let mut segments = Vec::new();
        push_segments(&path, &mut segments);
        unknown.push(segments.join("."));
    })?;
    Ok((value, unknown))
}

fn push_segments(path: &serde_ignored::Path, out: &mut Vec<String>) {
    use serde_ignored::Path;
    match path {
        Path::Root => {}
        Path::Seq { parent, index } => {
            push_segments(parent, out);
            out.push(index.to_string());
        }
        Path::Map { parent, key } => {
            push_segments(parent, out);
            out.push(key.clone());
        }
        Path::Some { parent }
        | Path::NewtypeStruct { parent }
        | Path::NewtypeVariant { parent } => push_segments(parent, out),
    }
}

/// The text to save `value` as, over the file's current text `old`. When
/// `old` is TOML that reads as `T`, it is edited in place: every key `value`
/// writes is set (unchanged values keep their formatting), keys `T` knows but
/// `value` no longer writes are removed (an old alias, an emptied optional),
/// and keys `T` doesn't know are left as they are, with all comments and
/// ordering. Otherwise (no file, or one that doesn't read) it is `value`
/// written out plainly.
pub fn merge_toml<T: Serialize + DeserializeOwned>(
    old: Option<&str>,
    value: &T,
) -> Result<String, toml::ser::Error> {
    let fresh = toml::to_string_pretty(value)?;
    let Some(old) = old else {
        return Ok(fresh);
    };
    let (Ok((_, unknown)), Ok(mut doc), Ok(new)) = (
        parse_with_unknown::<T>(old),
        old.parse::<DocumentMut>(),
        fresh.parse::<DocumentMut>(),
    ) else {
        return Ok(fresh);
    };
    let unknown: Vec<Vec<&str>> = unknown.iter().map(|k| k.split('.').collect()).collect();
    merge_table(
        doc.as_table_mut(),
        new.as_table(),
        &mut Vec::new(),
        &unknown,
    );
    Ok(doc.to_string())
}

/// Whether `path` is an unknown key or holds one.
fn holds_unknown(path: &[String], unknown: &[Vec<&str>]) -> bool {
    unknown
        .iter()
        .any(|u| u.len() >= path.len() && u.iter().zip(path).all(|(a, b)| a == b))
}

fn merge_table(
    dst: &mut dyn TableLike,
    src: &dyn TableLike,
    path: &mut Vec<String>,
    unknown: &[Vec<&str>],
) {
    let gone: Vec<String> = dst
        .iter()
        .map(|(k, _)| k.to_string())
        .filter(|k| !src.contains_key(k))
        .filter(|k| {
            path.push(k.clone());
            let keep = holds_unknown(path, unknown);
            path.pop();
            !keep
        })
        .collect();
    for k in gone {
        dst.remove(&k);
    }
    for (k, item) in src.iter() {
        if item.is_none() {
            continue;
        }
        match dst.get_mut(k) {
            Some(old) => {
                path.push(k.to_string());
                merge_item(old, item, path, unknown);
                path.pop();
            }
            None => {
                dst.insert(k, item.clone());
            }
        }
    }
}

fn merge_item(old: &mut Item, new: &Item, path: &mut Vec<String>, unknown: &[Vec<&str>]) {
    if let (Some(dst), Some(src)) = (old.as_table_like_mut(), new.as_table_like()) {
        return merge_table(dst, src, path, unknown);
    }
    match (old.as_value_mut(), new.as_value()) {
        (Some(o), Some(n)) => {
            if !same(o, n) {
                let decor = o.decor().clone();
                *o = n.clone();
                *o.decor_mut() = decor;
            }
        }
        // A different shape: a value where a table was, or the other way.
        // Inside an inline table everything has to stay a value.
        (Some(_), None) => {
            if let Ok(v) = new.clone().into_value() {
                *old = Item::Value(v);
            }
        }
        _ => *old = new.clone(),
    }
}

/// Equal as TOML values, whatever their formatting.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(x), Value::String(y)) => x.value() == y.value(),
        (Value::Integer(x), Value::Integer(y)) => x.value() == y.value(),
        (Value::Float(x), Value::Float(y)) => x.value().to_bits() == y.value().to_bits(),
        (Value::Boolean(x), Value::Boolean(y)) => x.value() == y.value(),
        (Value::Datetime(x), Value::Datetime(y)) => x.value() == y.value(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| same(a, b))
        }
        (Value::InlineTable(x), Value::InlineTable(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Settings {
        name: String,
        #[serde(alias = "old_size")]
        size: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        extra: Option<String>,
        inner: Inner,
    }

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Inner {
        on: bool,
    }

    #[test]
    fn unknown_keys_are_listed() {
        let (s, unknown) = parse_with_unknown::<Settings>(
            "name = \"a\"\nfuture = 1\nold_size = 3\n[inner]\non = true\nlater = [1]\n",
        )
        .unwrap();
        assert_eq!(s.size, 3, "an alias is known");
        assert!(s.inner.on);
        assert_eq!(unknown, ["future", "inner.later"]);
    }

    #[test]
    fn saving_keeps_unknown_keys_comments_and_order() {
        let old = "# my settings\nfuture = \"x\" # from a newer version\nname = 'a'   # keep\nold_size = 3\nextra = \"gone\"\n\n[inner] # inner table\nlater = 2\non = false\n";
        let value = Settings {
            name: "a".into(),
            size: 4,
            extra: None,
            inner: Inner { on: true },
        };
        let text = merge_toml(Some(old), &value).unwrap();
        assert_eq!(
            text,
            "# my settings\nfuture = \"x\" # from a newer version\nname = 'a'   # keep\nsize = 4\n\n[inner] # inner table\nlater = 2\non = true\n"
        );
        let (back, unknown) = parse_with_unknown::<Settings>(&text).unwrap();
        assert_eq!(back, value);
        assert_eq!(unknown, ["future", "inner.later"]);
    }

    #[test]
    fn new_keys_and_tables_go_at_the_end() {
        let old = "[inner]\non = true\n\n# about a table from a newer version\n[future]\nx = 1\n";
        let text = merge_toml(
            Some(old),
            &Settings {
                name: "b".into(),
                inner: Inner { on: true },
                ..Settings::default()
            },
        )
        .unwrap();
        assert_eq!(
            text,
            "name = \"b\"\nsize = 0\n[inner]\non = true\n\n# about a table from a newer version\n[future]\nx = 1\n"
        );
        // A table the file didn't have yet.
        let text = merge_toml(
            Some("name = \"b\"\n[future]\nx = 1\n"),
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(
            text,
            "name = \"\"\nsize = 0\n[future]\nx = 1\n\n[inner]\non = false\n"
        );
    }

    #[test]
    fn inline_tables_are_edited_in_place() {
        let old = "inner = { on = false, later = 1 }\n";
        let text = merge_toml(
            Some(old),
            &Settings {
                inner: Inner { on: true },
                ..Settings::default()
            },
        )
        .unwrap();
        let back: toml::Table = toml::from_str(&text).unwrap();
        assert_eq!(back["inner"]["on"].as_bool(), Some(true), "{text}");
        assert_eq!(back["inner"]["later"].as_integer(), Some(1), "{text}");
    }

    #[test]
    fn a_file_that_does_not_read_is_written_plainly() {
        let value = Settings::default();
        let plain = toml::to_string_pretty(&value).unwrap();
        assert_eq!(merge_toml(None, &value).unwrap(), plain);
        assert_eq!(merge_toml(Some("name = "), &value).unwrap(), plain);
        assert_eq!(merge_toml(Some("size = \"big\""), &value).unwrap(), plain);
    }
}
