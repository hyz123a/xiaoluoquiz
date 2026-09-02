use xiaoluoquiz::domain::{
    CandidateField, CandidateFieldConfig, CreatePaperInput, PaperMode, PaperQuestionInput,
    PaperValidationError,
};

fn valid_input() -> CreatePaperInput {
    CreatePaperInput {
        title: "Rust 基础考试".to_owned(),
        description: Some("固定试卷".to_owned()),
        audience: Some("软件工程班".to_owned()),
        mode: PaperMode::Exam,
        open_at: None,
        close_at: None,
        duration_seconds: Some(3_600),
        max_attempts: 1,
        allow_resume: true,
        auto_save: true,
        auto_submit: true,
        candidate_fields: vec![CandidateFieldConfig {
            key: CandidateField::StudentNumber,
            required: true,
        }],
        result_visibility: Default::default(),
        allow_preview: false,
        questions: vec![PaperQuestionInput {
            question_id: 1,
            score: Some(1.0),
        }],
    }
}

#[test]
fn paper_requires_a_non_empty_title() {
    let mut input = valid_input();
    input.title = "  ".to_owned();

    assert_eq!(input.validate(), Err(PaperValidationError::EmptyTitle));
}

#[test]
fn paper_requires_at_least_one_question() {
    let mut input = valid_input();
    input.questions.clear();

    assert_eq!(input.validate(), Err(PaperValidationError::EmptyQuestions));
}

#[test]
fn paper_rejects_duplicate_questions() {
    let mut input = valid_input();
    input.questions.push(PaperQuestionInput {
        question_id: 1,
        score: Some(2.0),
    });

    assert_eq!(
        input.validate(),
        Err(PaperValidationError::DuplicateQuestion(1))
    );
}

#[test]
fn paper_rejects_non_positive_or_non_finite_score_overrides() {
    let mut input = valid_input();
    input.questions[0].score = Some(0.0);
    assert_eq!(
        input.validate(),
        Err(PaperValidationError::InvalidQuestionScore)
    );

    input.questions[0].score = Some(f64::NAN);
    assert_eq!(
        input.validate(),
        Err(PaperValidationError::InvalidQuestionScore)
    );
}

#[test]
fn practice_papers_cannot_collect_candidate_information() {
    let mut input = valid_input();
    input.mode = PaperMode::Practice;

    assert_eq!(
        input.validate(),
        Err(PaperValidationError::PracticeCandidateFields)
    );
}

#[test]
fn exam_defaults_are_explicit_and_candidate_fields_are_preserved() {
    let input = valid_input();

    input
        .validate()
        .expect("valid paper should pass validation");
    assert_eq!(input.candidate_fields[0].key, CandidateField::StudentNumber);
    assert!(input.candidate_fields[0].required);
}

#[test]
fn paper_requires_an_explicit_score_for_each_question() {
    let mut input = valid_input();
    input.questions[0].score = None;

    assert_eq!(
        input.validate(),
        Err(PaperValidationError::InvalidQuestionScore)
    );
}
