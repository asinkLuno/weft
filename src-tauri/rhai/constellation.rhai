// 十二星座：计算只返回稳定 ID；归属名称和错误文字交给 humanizer。

fn sign_ids() {
    ["capricorn", "aquarius", "pisces", "aries", "taurus", "gemini",
     "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius"]
}

fn calculable_tag(ctx) {
    if ctx.date_mode != () {
        throw humanizer("calendar_error", ctx.date_mode, ctx.language);
    }
    let bt = ctx.entity.base_time;
    if bt == () {
        throw humanizer("missing_time", ctx.entity.id, ctx.language);
    }
    let month = bt[1];
    let day = bt[2];
    if month < 1 || month > 12 || day < 1 {
        throw humanizer("incomplete_time", ctx.entity.id, ctx.language);
    }
    let cutoff = [20, 19, 21, 20, 21, 22, 23, 23, 23, 24, 23, 22];
    let ids = sign_ids();
    let idx = (month - 1 + if day >= cutoff[month - 1] { 1 } else { 0 }) % 12;
    [ids[idx], "zodiac"]
}

fn humanizer(kind, value, lang) {
    let zh = lang == "zh";
    if kind == "chain" {
        let ids = sign_ids();
        let names = if zh {
            ["摩羯座", "水瓶座", "双鱼座", "白羊座", "金牛座", "双子座",
             "巨蟹座", "狮子座", "处女座", "天秤座", "天蝎座", "射手座"]
        } else {
            ["Capricorn", "Aquarius", "Pisces", "Aries", "Taurus", "Gemini",
             "Cancer", "Leo", "Virgo", "Libra", "Scorpio", "Sagittarius"]
        };
        let result = [];
        for id in value {
            if id == "zodiac" {
                result.push(if zh { "星座" } else { "Zodiac" });
                continue;
            }
            let index = -1;
            for i in 0..ids.len() {
                if ids[i] == id { index = i; break; }
            }
            if index < 0 { throw `unknown constellation: ${id}`; }
            result.push(names[index]);
        }
        return result;
    }
    if kind == "calendar_error" {
        return if zh {
            `计算星座需要公历，当前历法是 ${value}`
        } else {
            `constellation requires a Gregorian calendar, got ${value}`
        };
    }
    if kind == "missing_time" {
        return if zh {
            `实体 ${value} 没有 base_time，无法计算星座`
        } else {
            `entity ${value} has no base_time, cannot compute the constellation`
        };
    }
    if kind == "incomplete_time" {
        return if zh {
            `实体 ${value} 的 base_time 至少要写全 年月日，才能计算星座`
        } else {
            `entity ${value} needs at least year/month/day in base_time`
        };
    }
    throw `unknown humanizer kind: ${kind}`;
}
