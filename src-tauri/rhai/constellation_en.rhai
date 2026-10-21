// Built-in material: twelve constellations (English) — returns the belong_to chain [sign, "Zodiac"]

fn material(ctx) {
    // names[m] is the sign for the first half of month m+1; cutoff[m] is the switchover day
    let names = ["Capricorn", "Aquarius", "Pisces", "Aries", "Taurus", "Gemini",
                 "Cancer", "Leo", "Virgo", "Libra", "Scorpio", "Sagittarius"];
    let cutoff = [20, 19, 21, 20, 21, 22, 23, 23, 23, 24, 23, 22];
    if !ctx.date_mode.starts_with("gregorian") {
        throw `constellation requires a Gregorian calendar, got ${ctx.date_mode}`;
    }
    let bt = ctx.entity.base_time;
    if bt == () {
        throw `entity ${ctx.entity.id} has no base_time, cannot compute the constellation`;
    }
    let month = bt[1];
    let day = bt[2];
    if month < 1 || month > 12 || day < 1 {
        throw `entity ${ctx.entity.id} needs at least year/month/day in base_time`;
    }
    let idx = (month - 1 + if day >= cutoff[month - 1] { 1 } else { 0 }) % 12;
    [names[idx], "Zodiac"]
}
