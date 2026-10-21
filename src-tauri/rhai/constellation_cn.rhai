// 内置材料：十二星座（中文）——返回归属链 [星座, "星座"]：实体 belong_to 星座，星座 belong_to「星座」

fn material(ctx) {
    // names[m] 是 m+1 月上半月所属星座，cutoff[m] 是当月切换日（当天起归下半月星座）
    let names = ["摩羯座", "水瓶座", "双鱼座", "白羊座", "金牛座", "双子座",
                 "巨蟹座", "狮子座", "处女座", "天秤座", "天蝎座", "射手座"];
    let cutoff = [20, 19, 21, 20, 21, 22, 23, 23, 23, 24, 23, 22];
    if !ctx.date_mode.starts_with("gregorian") {
        throw `星座材料需要公历，当前历法是 ${ctx.date_mode}`;
    }
    let bt = ctx.entity.base_time;
    if bt == () {
        throw `实体 ${ctx.entity.id} 没有 base_time，无法计算星座`;
    }
    let month = bt[1];
    let day = bt[2];
    if month < 1 || month > 12 || day < 1 {
        throw `实体 ${ctx.entity.id} 的 base_time 至少要写全 年月日，才能计算星座`;
    }
    let idx = (month - 1 + if day >= cutoff[month - 1] { 1 } else { 0 }) % 12;
    [names[idx], "星座"]
}
