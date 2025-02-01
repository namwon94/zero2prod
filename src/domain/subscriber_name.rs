//'String'과 '&str'에 'graphemes' 메서드를 제공하기 위한 확장 트레이트
use unicode_segmentation::UnicodeSegmentation;

/*
    튜플 구조체 : String 타입의 단일 필드(이름이 없는)를 갖는 새로운 타입 
        -> String에 사용할 수 있는 어떤 메서드도 상속하지 않으며, 타입 변수에 String을 할당하려하면 컴파일 오류남
*/
#[derive(Debug)]
pub struct SubscriberName(String);

impl SubscriberName {
    //입력이 subscriber 이름에 대한 검증 조건을 모두 만족하면 'SubscriberName' 인스턴스를 반환한다.
    pub fn parse(s: String) -> Result<SubscriberName, String> {
        //'.trim()'은 입력 's'에 대해 뒤로 계속되는 공백 문자가 없는 뷰를 반환한다.
        //'.is_empty'는 해당 뷰가 문자를 포함하고 있는지 확인한다.
        let is_empty_or_whitespace = s.trim().is_empty();
        //grapheme는 "사용자가 인지할 수 있는" 문자로서 유니코드 표준에 의해 정의된다.
        //'a'는 단일 grapheme이지만, 두 개의 문자가 조합된 것이다. (a 와 *)
        //grapheme 입력 's'안의 grapheme에 대한 이터레이터를 반환한다.
        //'true'는 우리가 확장된 grapheme 정의 셋, 즉 권장되는 정의 셋을 사용하기 원함을 의미한다.
        let is_too_long = s.graphemes(true).count() > 256;

        //입력 's'의 모든 문자들에 대해 반복하면서 forbidden 배열  안에 있는 문자 중, 어느 하나와 일치하는 문자가 있는지 확인한다.
        let forbidden_characters = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
        let contains_forbidden_characters = s
            .chars()
            .any(|g| forbidden_characters.contains(&g));
        
        if is_empty_or_whitespace || is_too_long || contains_forbidden_characters {
            // 'panic'을 'Err()'으로 치환한다.
            //panic!("{} is not a valid subscriber name", s)
            Err(format!("{} is not a valid subscriber name.", s))
        }else {
            Ok(Self(s))
        }
        
    }
}

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriberName;
    use claim::{assert_err, assert_ok};

    #[test]
    fn a_256_grapheme_long_name_is_valid() {
        let name = "ë".repeat(256);
        assert_ok!(SubscriberName::parse(name));
    }

    #[test]
    fn a_name_longer_than_256_grapheme_is_rejected() {
        let name = "a".repeat(257);
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn whitespace_only_names_are_rejected() {
        let name = " ".to_string();
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn empty_string_is_rejected() {
        let name = "".to_string();
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn names_containing_an_invalid_character_are_rejected() {
        for name in &['/', '(', ')', '"', '<', '>', '\\', '{', '}',] {
            let name = name.to_string();
            assert_err!(SubscriberName::parse(name));
        }
    }

    #[test]
    fn a_valid_name_is_parsed_successfully() {
        let name = "Ursula Le Guin".to_string();
        assert_ok!(SubscriberName::parse(name));
    }
}