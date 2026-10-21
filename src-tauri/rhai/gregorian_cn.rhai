// 中文格里高利历本地化

fn metadata() {
    #{
        title: "格里高利历",
        description: "采用年、月、日和时分秒表示时间，并包含公历闰年规则。",
        components: 6,
        weekdays: ["一", "二", "三", "四", "五", "六", "日"]
    }
}

fn extra(values) {
    let names = ["一", "二", "三", "四", "五", "六", "日"];
    let n = weekday_number(values);
    let result = #{ weekday: names[n - 1], weekday_number: n };
    let y = values[0];
    let m = values[1];
    let d = values[2];
    if y >= 100 && m >= 1 && m <= 12 && d >= 1 && d <= 31 {
        let mothers_day = nth_weekday_of_month(y, 5, 7, 2);
        if m == 5 && d == mothers_day {
            result.holiday = "母亲节";
        }
        let fathers_day = nth_weekday_of_month(y, 6, 7, 3);
        if m == 6 && d == fathers_day {
            result.holiday = "父亲节";
        }
    }
    result
}

fn humanize(values) {
    let units = ["年", "月", "日", "时", "分", "秒"];
    let v = normalize(values);
    if v[0] == 0 && v[1] == 0 && v[2] == 0 && v[3] == 0 && v[4] == 0 && v[5] == 0 {
        return `0${units[0]}0${units[1]}0${units[2]}`;
    }
    let parts = [];
    for i in 0..6 {
        if v[i] != 0 {
            parts.push(`${v[i]}${units[i]}`);
        }
    }
    let out = "";
    for i in 0..parts.len() {
        out += parts[i];
    }
    out
}
