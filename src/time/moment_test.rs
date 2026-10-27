use std::time::Duration;

use super::Moment;

#[test]
fn the_span_between_two_readings_is_the_later_minus_the_earlier() {
    let earlier = Moment::since_origin(Duration::from_millis(250));
    let later = earlier.after(Duration::from_micros(1_234_567));

    assert_eq!(later.since(earlier), Duration::from_micros(1_234_567));
}

#[test]
fn a_reading_taken_before_the_other_reports_no_elapsed_time() {
    let earlier = Moment::since_origin(Duration::from_secs(1));
    let later = Moment::since_origin(Duration::from_secs(2));

    assert_eq!(earlier.since(later), Duration::ZERO);
    assert_eq!(later.since(later), Duration::ZERO);
}

#[test]
fn moving_forward_saturates_at_the_last_representable_reading() {
    let last = Moment::since_origin(Duration::MAX);

    assert_eq!(last.after(Duration::from_secs(1)), last);
}
