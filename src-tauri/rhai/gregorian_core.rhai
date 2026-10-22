// 格里高利历核心：历算逻辑，不包含语言/本地化相关内容

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
