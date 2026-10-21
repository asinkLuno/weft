// 格里高利历核心：历算逻辑，不包含语言/本地化相关内容

fn days_in_month(year, month) {
    if month == 2 {
        if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 { 29 } else { 28 }
    } else if month == 4 || month == 6 || month == 9 || month == 11 {
        30
    } else {
        31
    }
}

// shortcut: 月/日是 1 基，0 值不向高位借位（[2024,1,1,0,0,-1] 归一成 1 月 0 日）；
// 上游原样行为。做 drift 负偏移时改走 to_tick 加法，别依赖 normalize
fn normalize(values) {
    let result = values;
    // 月/日 1 基：上限本身合法（12月、当月末日），超出才进位；时分秒 0 基：达到上限即进位
    let limits = [0, 12, days_in_month(result[0], result[1]), 24, 60, 60];
    let idx = 5;
    while idx > 0 {
        let limit = limits[idx];
        let overflow = if idx <= 2 {
            result[idx] > limit
        } else {
            result[idx] >= limit
        };
        if limit > 0 && (overflow || result[idx] < 0) {
            let carry = if result[idx] >= 0 {
                result[idx] / limit
            } else {
                (result[idx] - limit + 1) / limit
            };
            result[idx - 1] += carry;
            result[idx] -= carry * limit;
        }
        idx -= 1;
    }
    result
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
