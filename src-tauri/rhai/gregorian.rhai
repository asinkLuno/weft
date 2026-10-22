// 格里高利历：历算只处理数字；所有展示由 humanizer(kind, value, language) 处理。

fn metadata() { #{ components: 6 } }

// 反算 tick；用整段日期换算处理跨月、跨年及负的时分秒。
fn from_tick(tick) {
    let days = tick / 86400;
    let seconds = tick % 86400;
    if seconds < 0 {
        days -= 1;
        seconds += 86400;
    }
    let era = if days >= 0 { days / 146097 } else { (days - 146096) / 146097 };
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    [year, month, day, seconds / 3600, seconds % 3600 / 60, seconds % 60]
}

fn normalize(values) {
    let result = values;
    // 0 月/0 日表示未指定，保留展示精度；只有实际进位时才补出日期。
    if result[1] != 0 {
        let month_idx = (result[1] - 1) % 12;
        if month_idx < 0 { month_idx += 12; }
        result[0] += (result[1] - 1 - month_idx) / 12;
        result[1] = month_idx + 1;
    }
    if result[2] == 0 && result[3] >= 0 && result[3] < 24
        && result[4] >= 0 && result[4] < 60 && result[5] >= 0 && result[5] < 60 {
        return result;
    }
    if result[1] == 0 { result[1] = 1; }
    if result[2] == 0 { result[2] = 1; }
    from_tick(to_tick(result))
}

fn to_tick(values) {
    let year = values[0];
    let month = values[1];
    let day = values[2];
    let hour = values[3];
    let minute = values[4];
    let second = values[5];

    let raw_month_idx = (month - 1) % 12;
    let month_idx = if raw_month_idx < 0 { raw_month_idx + 12 } else { raw_month_idx };
    let year_carry = ((month - 1) - month_idx) / 12;
    year += year_carry;
    month = month_idx + 1;

    let total_seconds = hour * 3600 + minute * 60 + second;
    let day_carry = total_seconds / 86400;
    let seconds = total_seconds % 86400;

    let adjusted_year = year - if month <= 2 { 1 } else { 0 };
    let era = if adjusted_year >= 0 {
        adjusted_year / 400
    } else {
        (adjusted_year - 399) / 400
    };
    let yoe = adjusted_year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * shifted_month + 2) / 5;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy + day - 1 + day_carry;
    (era * 146097 + doe) * 86400 + seconds
}

/// Return 1-7 where 1=Monday … 7=Sunday (Sakamoto's algorithm).
fn weekday_number(values) {
    let y = values[0];
    let m = values[1];
    let d = values[2];
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    if m < 3 { y -= 1; }
    let sakamoto = (y + y / 4 - y / 100 + y / 400 + t[m - 1] + d) % 7;
    if sakamoto == 0 { 7 } else { sakamoto }
}

/// Day of month for the Nth occurrence of target_weekday (1=Mon … 7=Sun).
fn nth_weekday_of_month(year, month, target_weekday, n) {
    let first = weekday_number([year, month, 1]);
    let first_target = 1 + (target_weekday - first + 7) % 7;
    first_target + (n - 1) * 7
}

fn extra(values) {
    let n = weekday_number(values);
    let result = #{ weekday_number: n, holiday: "" };
    let y = values[0];
    let m = values[1];
    let d = values[2];
    if y >= 100 && m >= 1 && m <= 12 && d >= 1 && d <= 31 {
        if m == 5 && d == nth_weekday_of_month(y, 5, 7, 2) {
            result.holiday = "mothers_day";
        } else if m == 6 && d == nth_weekday_of_month(y, 6, 7, 3) {
            result.holiday = "fathers_day";
        } else if m == 11 && d == nth_weekday_of_month(y, 11, 4, 4) {
            result.holiday = "thanksgiving";
        }
    }
    result
}

fn duration_parts(seconds) {
    let sign = if seconds < 0 { "-" } else { "" };
    let days = if seconds < 0 { -(seconds / 86400) } else { seconds / 86400 };
    let remaining = if seconds < 0 { -(seconds % 86400) } else { seconds % 86400 };
    #{ sign: sign, amounts: [days, remaining / 3600, remaining % 3600 / 60, remaining % 60] }
}

fn humanizer(kind, value, lang) {
    let zh = lang == "zh";
    if kind == "metadata" {
        let result = value;
        result.title = if zh { "格里高利历" } else { "Gregorian Calendar" };
        result.description = if zh {
            "采用年、月、日和时分秒表示时间，并包含公历闰年规则。"
        } else {
            "Represents time with years, months, days, hours, minutes, and seconds."
        };
        result.weekdays = if zh {
            ["一", "二", "三", "四", "五", "六", "日"]
        } else {
            ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        };
        return result;
    }
    if kind == "extra" {
        let result = #{ weekday_number: value.weekday_number };
        let days = if zh {
            ["一", "二", "三", "四", "五", "六", "日"]
        } else {
            ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        };
        result.weekday = days[result.weekday_number - 1];
        if value.holiday == "mothers_day" {
            result.holiday = if zh { "母亲节" } else { "Mother's Day" };
        } else if value.holiday == "fathers_day" {
            result.holiday = if zh { "父亲节" } else { "Father's Day" };
        } else if value.holiday == "thanksgiving" {
            result.holiday = if zh { "感恩节" } else { "Thanksgiving" };
        }
        return result;
    }
    if kind == "duration" {
        let duration = duration_parts(value);
        let amounts = duration.amounts;
        let cn_units = ["日", "时", "分", "秒"];
        let en_units = ["day", "hour", "minute", "second"];
        let parts = [];
        for i in 0..4 {
            if amounts[i] != 0 {
                if zh {
                    parts.push(`${amounts[i]}${cn_units[i]}`);
                } else {
                    let plural = if amounts[i] == 1 { "" } else { "s" };
                    parts.push(`${amounts[i]} ${en_units[i]}${plural}`);
                }
            }
        }
        if parts.len() == 0 { return if zh { "0秒" } else { "0 seconds" }; }
        let out = duration.sign;
        for i in 0..parts.len() {
            if !zh && i > 0 { out += ", "; }
            out += parts[i];
        }
        return out;
    }
    if kind == "time" {
        let v = normalize(value);
        if v[0] == 0 && v[1] == 0 && v[2] == 0 && v[3] == 0 && v[4] == 0 && v[5] == 0 {
            return if zh { "0年0月0日" } else { "0 years, 0 months, 0 days" };
        }
        if zh {
            let units = ["年", "月", "日", "时", "分", "秒"];
            let out = "";
            for i in 0..6 {
                if v[i] != 0 { out += `${v[i]}${units[i]}`; }
            }
            return out;
        }
        let months = ["Jan.", "Feb.", "Mar.", "Apr.", "May", "Jun.", "Jul.", "Aug.", "Sep.", "Oct.", "Nov.", "Dec."];
        if v[0] >= 100 && v[2] == 0 && v[3] == 0 && v[4] == 0 && v[5] == 0 {
            if v[1] == 0 { return `${v[0]}`; }
            if v[1] >= 1 && v[1] <= 12 { return `${months[v[1] - 1]} ${v[0]}`; }
        }
        if v[0] >= 100 && v[1] >= 1 && v[1] <= 12 && v[2] >= 1 && v[2] <= 31 {
            let date = `${months[v[1] - 1]} ${v[2]}, ${v[0]}`;
            if v[3] == 0 && v[4] == 0 && v[5] == 0 { return date; }
            let hh = if v[3] < 10 { `0${v[3]}` } else { `${v[3]}` };
            let mm = if v[4] < 10 { `0${v[4]}` } else { `${v[4]}` };
            let ss = if v[5] < 10 { `0${v[5]}` } else { `${v[5]}` };
            return `${date}, ${hh}:${mm}:${ss}`;
        }
        let units = ["year", "month", "day", "hour", "minute", "second"];
        let parts = [];
        for i in 0..6 {
            if v[i] != 0 {
                let plural = if v[i] == 1 { "" } else { "s" };
                parts.push(`${v[i]} ${units[i]}${plural}`);
            }
        }
        let out = "";
        for i in 0..parts.len() {
            if i > 0 { out += ", "; }
            out += parts[i];
        }
        return out;
    }
    throw `unknown humanizer kind: ${kind}`;
}
