use wezterm_bidi::*;

fn class_by_name(s: &str) -> BidiClass {
    match s {
        "AL" => BidiClass::ArabicLetter,
        "AN" => BidiClass::ArabicNumber,
        "BN" => BidiClass::BoundaryNeutral,
        "CS" => BidiClass::CommonSeparator,
        "EN" => BidiClass::EuropeanNumber,
        "ES" => BidiClass::EuropeanSeparator,
        "ET" => BidiClass::EuropeanTerminator,
        "FSI" => BidiClass::FirstStrongIsolate,
        "L" => BidiClass::LeftToRight,
        "LRO" => BidiClass::LeftToRightOverride,
        "LRE" => BidiClass::LeftToRightEmbedding,
        "LRI" => BidiClass::LeftToRightIsolate,
        "NSM" => BidiClass::NonspacingMark,
        "ON" => BidiClass::OtherNeutral,
        "B" => BidiClass::ParagraphSeparator,
        "PDF" => BidiClass::PopDirectionalFormat,
        "PDI" => BidiClass::PopDirectionalIsolate,
        "R" => BidiClass::RightToLeft,
        "RLE" => BidiClass::RightToLeftEmbedding,
        "RLI" => BidiClass::RightToLeftIsolate,
        "RLO" => BidiClass::RightToLeftOverride,
        "S" => BidiClass::SegmentSeparator,
        "WS" => BidiClass::WhiteSpace,
        bad => panic!("invalid BidiClass {}", bad),
    }
}

fn parse_codepoint(s: &str) -> u32 {
    u32::from_str_radix(s.trim(), 16).unwrap()
}

#[test]
fn bidi_character_test() {
    let _ = env_logger::Builder::new().is_test(true).try_init();

    let data = include_str!("../data/BidiCharacterTest.txt");

    // This helps to iterate on regressions by skipping over tests
    // until we reach the line number we're testing
    let first_line = 0;
    let mut levels: Vec<Level> = vec![];
    let mut reorder: Vec<usize> = vec![];
    let mut context = BidiContext::new();
    let mut level_passes = 0;
    let mut level_fails = 0;
    let mut para_passes = 0;
    let mut para_fails = 0;
    let mut reorder_passes = 0;
    let mut reorder_fails = 0;

    for (line_number, line) in data.lines().enumerate() {
        if line_number + 1 < first_line {
            continue;
        }

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split(';').collect();
        let codepoints: Vec<char> = fields[0]
            .split_whitespace()
            .map(parse_codepoint)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();

        let direction: i8 = fields[1].parse().unwrap();
        let para_level: Level = Level(fields[2].parse().unwrap());

        levels.clear();
        for field in fields[3].split_whitespace() {
            if field == "x" {
                levels.push(Level(NO_LEVEL));
            } else {
                levels.push(Level(field.parse().unwrap()));
            }
        }

        reorder.clear();
        for field in fields[4].split_whitespace() {
            reorder.push(field.parse().unwrap());
        }

        log::debug!("BidiCharacterTest.txt:{}", line_number + 1);
        log::debug!("{:?}", codepoints);

        context.resolve_paragraph(
            &codepoints,
            match direction {
                0 => ParagraphDirectionHint::LeftToRight,
                1 => ParagraphDirectionHint::RightToLeft,
                2 => ParagraphDirectionHint::AutoLeftToRight,
                _ => panic!("invalid direction code {}", direction),
            },
        );

        if context.base_level() != para_level {
            log::error!(
                "\nBidiCharacterTest.txt:{}\n   {:?}\n   base_level={:?} expected={:?}",
                line_number + 1,
                codepoints,
                context.base_level(),
                para_level,
            );
            para_fails += 1;
        } else {
            para_passes += 1;
        }

        let (resolved_levels, actual_reordered) = context.reorder_line(0..codepoints.len());

        if resolved_levels != levels {
            log::error!(
                "\nBidiCharacterTest.txt:{}\n   {:?}\n   expected={:?}",
                line_number + 1,
                codepoints,
                levels
            );
            log::error!("     levels={:?}", resolved_levels);
            level_fails += 1;
        } else {
            level_passes += 1;
        }

        if actual_reordered != reorder {
            log::error!(
                "\nBidiCharacterTest.txt:{}\n   visual={:?}\n   expected={:?}",
                line_number + 1,
                actual_reordered,
                reorder
            );
            reorder_fails += 1;
        } else {
            reorder_passes += 1;
        }

        if reorder_fails + level_fails + para_fails > 0 {
            log::error!("{:#?}", context);
            break;
        }
    }

    println!("level_passes={} level_fails={}", level_passes, level_fails);
    println!("para_passes={} para_fails={}", para_passes, para_fails);
    println!(
        "reorder_passes={} reorder_fails={}",
        reorder_passes, reorder_fails
    );
    assert_eq!(level_fails + para_fails + reorder_fails, 0);
    assert_eq!(level_passes, 91707);
    assert_eq!(reorder_passes, 91707);
}

#[test]
fn bidi_test() {
    let _ = env_logger::Builder::new().is_test(true).try_init();
    let data = include_str!("../data/BidiTest.txt");

    let mut levels: Vec<Level> = vec![];
    let mut reorder: Vec<usize> = vec![];
    let mut context = BidiContext::new();

    let mut level_passes = 0;
    let mut level_fails = 0;
    let mut reorder_passes = 0;
    let mut reorder_fails = 0;

    // This helps to iterate on regressions by skipping over tests
    // until we reach the line number we're testing
    let first_line = 0;

    for (line_number, line) in data.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("@Levels:") {
            levels.clear();
            for field in line.split_whitespace().skip(1) {
                if field == "x" {
                    levels.push(Level(NO_LEVEL));
                } else {
                    levels.push(Level(field.parse().unwrap()));
                }
            }
            continue;
        }
        if line.starts_with("@Reorder:") {
            reorder.clear();
            for field in line.split_whitespace().skip(1) {
                reorder.push(field.parse().unwrap());
            }
            continue;
        }
        if line.starts_with('@') {
            continue;
        }

        let fields: Vec<&str> = line.split(';').collect();

        let inputs: Vec<BidiClass> = fields[0].split_whitespace().map(class_by_name).collect();
        let bitset: u32 = fields[1].trim().parse().unwrap();

        let mut directions: Vec<ParagraphDirectionHint> = vec![];
        if bitset & 1 == 1 {
            directions.push(ParagraphDirectionHint::AutoLeftToRight);
        }
        if bitset & 2 == 2 {
            directions.push(ParagraphDirectionHint::LeftToRight);
        }
        if bitset & 4 == 4 {
            directions.push(ParagraphDirectionHint::RightToLeft);
        }

        let mut printed_summary = false;

        if line_number < first_line {
            continue;
        }

        for &dir in &directions {
            context.set_char_types(&inputs, dir);
            let (resolved_levels, actual_reordered) = context.reorder_line(0..inputs.len());
            if resolved_levels != levels {
                if !printed_summary {
                    log::error!(
                        "\nBidiTest.txt:{}: {:?}\n   {:?}\n   expected={:?}",
                        line_number + 1,
                        directions,
                        inputs,
                        levels
                    );
                    printed_summary = true;
                }
                log::error!("   {:?} levels={:?}", dir, resolved_levels);
                log::error!("{:#?}", context);
                level_fails += 1;
            } else {
                level_passes += 1;
            }

            if actual_reordered != reorder {
                log::error!(
                    "\nBidiTest.txt:{}: {:?}\n   visual={:?}\n   expected={:?}",
                    line_number + 1,
                    directions,
                    actual_reordered,
                    reorder
                );
                reorder_fails += 1;
            } else {
                reorder_passes += 1;
            }
        }

        if level_fails + reorder_fails > 0 {
            log::error!("Stopping tests to limit output: too many failures");

            break;
        }
    }

    println!("levels: {} passed, {} failed", level_passes, level_fails);
    println!(
        "reorders: {} passed, {} failed",
        reorder_passes, reorder_fails
    );

    assert_eq!(level_fails, 0);
    assert_eq!(level_passes, 770241);

    assert_eq!(reorder_fails, 0);
    assert_eq!(reorder_passes, 770241);
}
