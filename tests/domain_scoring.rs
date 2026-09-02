use xiaoluoquiz::domain::{AnswerPayload, CorrectAnswer, EvaluationStatus, evaluate_answer};

#[test]
fn single_choice_answer_is_evaluated_by_the_domain() {
    let result = evaluate_answer(
        &AnswerPayload::SingleChoice {
            option_key: "B".to_owned(),
        },
        &CorrectAnswer::SingleChoice {
            option_key: "B".to_owned(),
        },
    )
    .expect("matching choice should be valid");

    assert_eq!(result.status, EvaluationStatus::Correct);
    assert_eq!(result.score, Some(1.0));
}

#[test]
fn multiple_choice_answer_is_evaluated_as_an_order_independent_set() {
    let result = evaluate_answer(
        &AnswerPayload::MultipleChoice {
            option_keys: vec!["E".to_owned(), "A".to_owned(), "C".to_owned()],
        },
        &CorrectAnswer::MultipleChoice {
            option_keys: vec!["A".to_owned(), "C".to_owned(), "E".to_owned()],
        },
    )
    .expect("matching multiple-choice answer should be valid");

    assert_eq!(result.status, EvaluationStatus::Correct);
    assert_eq!(result.score, Some(1.0));

    let result = evaluate_answer(
        &AnswerPayload::MultipleChoice {
            option_keys: vec!["A".to_owned(), "C".to_owned()],
        },
        &CorrectAnswer::MultipleChoice {
            option_keys: vec!["A".to_owned(), "C".to_owned(), "E".to_owned()],
        },
    )
    .expect("non-matching multiple-choice answer should be valid");

    assert_eq!(result.status, EvaluationStatus::Incorrect);
    assert_eq!(result.score, Some(0.0));
}
