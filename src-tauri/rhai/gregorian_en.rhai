// English Gregorian localization

fn metadata() {
    #{
        title: "Gregorian Calendar",
        description: "Represents time with years, months, days, hours, minutes, and seconds.",
        components: 6,
        weekdays: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    }
}

const MONTHS = ["Jan.", "Feb.", "Mar.", "Apr.", "May", "Jun.", "Jul.", "Aug.", "Sep.", "Oct.", "Nov.", "Dec."];

fn extra(values) {
    let names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let n = weekday_number(values);
    let result = #{ weekday: names[n - 1], weekday_number: n };
    let y = values[0];
    let m = values[1];
    let d = values[2];
    if y >= 100 && m >= 1 && m <= 12 && d >= 1 && d <= 31 {
        let mothers_day = nth_weekday_of_month(y, 5, 7, 2);
        if m == 5 && d == mothers_day {
            result.holiday = "Mother's Day";
        }
        let fathers_day = nth_weekday_of_month(y, 6, 7, 3);
        if m == 6 && d == fathers_day {
            result.holiday = "Father's Day";
        }
        let thanksgiving = nth_weekday_of_month(y, 11, 4, 4);
        if m == 11 && d == thanksgiving {
            result.holiday = "Thanksgiving";
        }
    }
    result
}

fn humanize(values) {
    let v = normalize(values);

    if v[0] == 0 && v[1] == 0 && v[2] == 0 && v[3] == 0 && v[4] == 0 && v[5] == 0 {
        return "0 years, 0 months, 0 days";
    }

    // Absolute date: year >= 100 and month in 1..12 → "Mon DD, YYYY"
    if v[0] >= 100 && v[1] >= 1 && v[1] <= 12 && v[2] >= 1 && v[2] <= 31 {
        let date = `${MONTHS[v[1] - 1]} ${v[2]}, ${v[0]}`;
        if v[3] == 0 && v[4] == 0 && v[5] == 0 {
            return date;
        }
        let hh = if v[3] < 10 { `0${v[3]}` } else { `${v[3]}` };
        let mm = if v[4] < 10 { `0${v[4]}` } else { `${v[4]}` };
        let ss = if v[5] < 10 { `0${v[5]}` } else { `${v[5]}` };
        return `${date}, ${hh}:${mm}:${ss}`;
    }

    // Duration / offset → "X years, Y months, Z days"
    let units = ["year", "month", "day", "hour", "minute", "second"];
    let parts = [];
    for i in 0..6 {
        if v[i] != 0 {
            if v[i] == 1 {
                parts.push(`${v[i]} ${units[i]}`);
            } else {
                parts.push(`${v[i]} ${units[i]}s`);
            }
        }
    }
    let out = "";
    for i in 0..parts.len() {
        if i > 0 {
            out += ", ";
        }
        out += parts[i];
    }
    out
}
