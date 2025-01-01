//! This crate provides some helpers to automatically tabulate data
//! so that it is presented reasonably nicely for humans to read,
//! without requiring that each column be hard coded to particular
//! widths in the code beforehand.
use termwiz::cell::{unicode_column_width, CellAttributes};
use termwiz::surface::Change;

/// Describes the alignment of a column
#[derive(Debug, Clone, Copy)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

/// Describes a column
#[derive(Debug)]
pub struct Column {
    /// The name of the column; this is the column header text
    pub name: String,
    /// How the column should be aligned
    pub alignment: Alignment,
}

fn emit_column<W: std::io::Write>(
    text: &str,
    max_width: usize,
    alignment: Alignment,
    output: &mut W,
) -> Result<(), std::io::Error> {
    let text_width = unicode_column_width(text, None);
    let (left_pad, right_pad) = match alignment {
        Alignment::Left => (0, max_width - text_width),
        Alignment::Center => {
            let left_pad = (max_width - text_width) / 2;
            // for odd-length columns, take care to use the remaining
            // length rather than just assuming that the right_pad
            // will have the same value as the left_pad
            let right_pad = max_width - (text_width + left_pad);
            (left_pad, right_pad)
        }
        Alignment::Right => (max_width - text_width, 0),
    };

    for _ in 0..left_pad {
        write!(output, " ")?;
    }
    write!(output, "{}", text)?;
    for _ in 0..right_pad {
        write!(output, " ")?;
    }

    Ok(())
}

/// Given a set of column headers and the row content,
/// automatically compute the column widths and then format
/// the data to the output stream.
/// If a given row has more columns than are defined in the
/// columns slice, then a left aligned column with no label
/// will be assumed.
pub fn tabulate_output<S: std::string::ToString, W: std::io::Write>(
    columns: &[Column],
    rows: &[Vec<S>],
    output: &mut W,
) -> Result<(), std::io::Error> {
    let mut col_widths: Vec<usize> = columns
        .iter()
        .map(|c| unicode_column_width(&c.name, None))
        .collect();

    let mut display_rows: Vec<Vec<String>> = vec![];
    for src_row in rows {
        let dest_row: Vec<String> = src_row.iter().map(|col| col.to_string()).collect();
        for (idx, col) in dest_row.iter().enumerate() {
            let col_width = unicode_column_width(col, None);
            if let Some(width) = col_widths.get_mut(idx) {
                *width = (*width).max(col_width);
            } else {
                col_widths.push(col_width);
            }
        }
        display_rows.push(dest_row);
    }

    for (idx, col) in columns.iter().enumerate() {
        if idx > 0 {
            write!(output, " ")?;
        }

        emit_column(&col.name, col_widths[idx], col.alignment, output)?;
    }
    writeln!(output)?;

    for row in &display_rows {
        for (idx, col) in row.iter().enumerate() {
            let max_width = col_widths.get(idx).cloned().unwrap_or(col.len());
            let alignment = columns
                .get(idx)
                .map(|c| c.alignment)
                .unwrap_or(Alignment::Left);

            if idx > 0 {
                write!(output, " ")?;
            }

            emit_column(col, max_width, alignment, output)?;
        }
        writeln!(output)?;
    }

    Ok(())
}

pub fn unicode_column_width_of_change_slice(s: &[Change]) -> usize {
    s.iter()
        .map(|c| {
            if c.is_text() {
                unicode_column_width(c.text(), None)
            } else {
                0
            }
        })
        .sum()
}

fn emit_padding_for_terminal(len: usize, spacer: &CellAttributes, output: &mut Vec<Change>) {
    if len == 0 {
        return;
    }
    output.push(Change::AllAttributes(spacer.clone()));
    let mut s = String::new();
    for _ in 0..len {
        s.push(' ');
    }
    output.push(s.into());
}

fn emit_column_for_terminal(
    s: &[Change],
    max_width: usize,
    alignment: Alignment,
    output: &mut Vec<Change>,
    spacer: &CellAttributes,
) {
    let text_width = unicode_column_width_of_change_slice(s);
    let (left_pad, right_pad) = match alignment {
        Alignment::Left => (0, max_width - text_width),
        Alignment::Center => {
            let left_pad = (max_width - text_width) / 2;
            // for odd-length columns, take care to use the remaining
            // length rather than just assuming that the right_pad
            // will have the same value as the left_pad
            let right_pad = max_width - (text_width + left_pad);
            (left_pad, right_pad)
        }
        Alignment::Right => (max_width - text_width, 0),
    };

    emit_padding_for_terminal(left_pad, spacer, output);
    output.extend_from_slice(s);
    emit_padding_for_terminal(right_pad, spacer, output);
}

pub fn tabulate_for_terminal(
    columns: &[Column],
    rows: &[Vec<Vec<Change>>],
    spacer: CellAttributes,
    result: &mut Vec<Change>,
) {
    let mut col_widths: Vec<usize> = columns
        .iter()
        .map(|c| unicode_column_width(&c.name, None))
        .collect();

    for row in rows {
        for (idx, col) in row.iter().enumerate() {
            let col_width = unicode_column_width_of_change_slice(col);
            if let Some(width) = col_widths.get_mut(idx) {
                *width = (*width).max(col_width);
            } else {
                col_widths.push(col_width);
            }
        }
    }

    for (idx, col) in columns.iter().enumerate() {
        if idx > 0 {
            emit_padding_for_terminal(1, &spacer, result);
        }
        emit_column_for_terminal(
            &[col.name.clone().into()],
            col_widths[idx],
            col.alignment,
            result,
            &spacer,
        );
    }
    result.push("\r\n".into());

    for row in rows {
        for (idx, col) in row.iter().enumerate() {
            let max_width = col_widths.get(idx).cloned().unwrap_or(col.len());
            let alignment = columns
                .get(idx)
                .map(|c| c.alignment)
                .unwrap_or(Alignment::Left);

            if idx > 0 {
                emit_padding_for_terminal(1, &spacer, result);
            }

            emit_column_for_terminal(col, max_width, alignment, result, &spacer);
        }
        result.push("\r\n".into());
    }
}

/// A convenience around `tabulate_output` that returns a String holding
/// the formatted data.
pub fn tabulate_output_as_string<S: std::string::ToString>(
    columns: &[Column],
    rows: &[Vec<S>],
) -> Result<String, std::io::Error> {
    let mut output: Vec<u8> = vec![];
    tabulate_output(columns, rows, &mut output)?;
    String::from_utf8(output)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e)))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basics() {
        let cols = vec![
            Column {
                name: "hello".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "middle-of-me".to_string(),
                alignment: Alignment::Center,
            },
            Column {
                name: "world".to_string(),
                alignment: Alignment::Right,
            },
        ];
        let data = vec![vec!["one", "i", "two"], vec!["longer", "boo", "again"]];

        let output = tabulate_output_as_string(&cols, &data).unwrap();
        eprintln!("output is:\n{}", output);
        assert_eq!(
            output,
            "hello  middle-of-me world\n\
             one         i         two\n\
             longer     boo      again\n"
        );
    }
}
