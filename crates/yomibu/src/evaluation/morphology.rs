use crate::analysis::Token;

pub(crate) fn reading_matches(reading: &str, token: &Token, text: &str) -> bool {
    let Some(surface) = text.get(token.span.clone()) else {
        return false;
    };
    if token.part_of_speech[0] == "動詞" && surface != token.dictionary_form {
        regular_stem(reading, token).is_some_and(|reading| reading == token.reading)
    } else {
        reading == token.reading
    }
}

pub(crate) fn regular_stem(base: &str, token: &Token) -> Option<String> {
    let class = &token.part_of_speech[4];
    if class.starts_with("五段-") {
        godan_stem(base)
    } else if class.starts_with("上一段-") || class.starts_with("下一段-") {
        base.strip_suffix(['る', 'ル']).map(str::to_owned)
    } else {
        None
    }
}

fn godan_stem(base: &str) -> Option<String> {
    let (index, last) = base.char_indices().next_back()?;
    let replacement = match last {
        'う' => 'い',
        'く' => 'き',
        'ぐ' => 'ぎ',
        'す' => 'し',
        'つ' => 'ち',
        'ぬ' => 'に',
        'ぶ' => 'び',
        'む' => 'み',
        'る' => 'り',
        'ウ' => 'イ',
        'ク' => 'キ',
        'グ' => 'ギ',
        'ス' => 'シ',
        'ツ' => 'チ',
        'ヌ' => 'ニ',
        'ブ' => 'ビ',
        'ム' => 'ミ',
        'ル' => 'リ',
        _ => return None,
    };
    Some(format!("{}{replacement}", &base[..index]))
}

pub(crate) fn supports_target_morphology(token: &Token, text: &str) -> bool {
    match token.part_of_speech[0].as_str() {
        "名詞" | "代名詞" => true,
        "動詞" => {
            let Some(stem) = regular_stem(&token.dictionary_form, token) else {
                return false;
            };
            let Some(surface) = text.get(token.span.clone()) else {
                return false;
            };
            surface == token.dictionary_form || surface == stem
        }
        _ => false,
    }
}
