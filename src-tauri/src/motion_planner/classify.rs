#[derive(Clone, Copy)]
pub(super) struct PhaseDefinition {
    pub(super) name: &'static str,
    pub(super) description: &'static str,
    pub(super) weight: f64,
}

fn contains_word(text: &str, expected: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .any(|word| word == expected)
}

pub(super) fn classify_motion(
    prompt: &str,
) -> (&'static str, u32, Vec<PhaseDefinition>, bool, bool) {
    let lower = prompt.to_ascii_lowercase();
    if ["death", "dying", "collapse", "defeat"]
        .iter()
        .any(|word| contains_word(&lower, word))
    {
        return (
            "a readable one-shot death",
            10,
            vec![
                phase("Alive", "Establish the final controlled pose", 0.8),
                phase("Reaction", "Show the source and direction of force", 0.8),
                phase(
                    "Loss of balance",
                    "Break the standing center of gravity",
                    0.9,
                ),
                phase("Descent", "Carry the silhouette toward the ground", 0.8),
                phase("Impact", "Show the body reaching the ground", 1.0),
                phase(
                    "Compression",
                    "Absorb the impact without changing identity",
                    0.8,
                ),
                phase("Settle", "Resolve secondary motion", 1.0),
                phase("Rest", "Hold the final readable state", 1.3),
            ],
            false,
            false,
        );
    }
    if ["dodge", "roll", "evade", "sidestep"]
        .iter()
        .any(|word| contains_word(&lower, word))
    {
        return (
            "a fast evasive action",
            6,
            vec![
                phase(
                    "Ready",
                    "NEAR and FAR legs grounded shoulder width; establish the starting pose",
                    0.8,
                ),
                phase(
                    "Anticipation",
                    "Compress opposite the escape direction; NEAR leg loads",
                    0.8,
                ),
                phase(
                    "Launch",
                    "NEAR leg pushes the center of mass into the dodge while the FAR leg trails",
                    0.6,
                ),
                phase(
                    "Travel",
                    "Show the clearest evasive silhouette; FAR leg tucks behind the NEAR leg",
                    0.6,
                ),
                phase(
                    "Landing",
                    "FAR leg reconnects to the ground first and absorbs the landing",
                    0.8,
                ),
                phase(
                    "Recovery",
                    "Both legs regrounded under the body in a controllable pose",
                    1.0,
                ),
            ],
            false,
            true,
        );
    }
    if contains_word(&lower, "cast")
        || contains_word(&lower, "casting")
        || contains_word(&lower, "spell")
    {
        return (
            "a staged spell cast",
            9,
            vec![
                phase("Ready", "Establish the caster and focus", 0.9),
                phase("Anticipation", "Draw the hands or focus inward", 0.8),
                phase("Gather", "Build readable magical energy", 1.0),
                phase("Charge", "Increase energy and secondary motion", 1.0),
                phase("Release", "Show the decisive casting gesture", 0.7),
                phase("Flash", "Hold the clearest effect contact", 0.8),
                phase("Follow-through", "Carry the gesture past release", 0.8),
                phase("Recovery", "Settle character and effect remnants", 1.0),
                phase("Return", "Reach the final stable pose", 0.9),
            ],
            false,
            false,
        );
    }
    let heavy = ["heavy", "greatsword", "spinning", "combo"]
        .iter()
        .any(|word| contains_word(&lower, word));
    if heavy {
        return (
            "a multi-stage heavy action",
            12,
            vec![
                phase("Start", "Establish the readable starting pose", 0.8),
                phase("Anticipation", "Shift weight opposite the action", 1.1),
                phase("Wind-up", "Load the body and weapon", 1.2),
                phase("Acceleration", "Begin the committed movement", 0.8),
                phase("Primary action", "Carry the fastest readable arc", 0.7),
                phase("Impact", "Show contact and maximum force", 1.1),
                phase("Secondary action", "Continue the combo or spin", 0.9),
                phase("Follow-through", "Resolve momentum", 1.0),
                phase("Recovery", "Return balance and silhouette", 1.1),
                phase("Return", "Reconnect cleanly to the start", 0.8),
            ],
            false,
            false,
        );
    }
    if ["walk", "walking"]
        .iter()
        .any(|word| contains_word(&lower, word))
    {
        return (
            "a looping walk cycle",
            8,
            vec![
                phase(
                    "Near contact",
                    "NEAR leg plants ahead under the hip at heel strike; FAR leg trails behind on the toe",
                    1.0,
                ),
                phase(
                    "Near down",
                    "Weight compresses over the NEAR planted foot; FAR heel lifts",
                    0.9,
                ),
                phase(
                    "Near passing",
                    "FAR leg swings through beside the NEAR planted leg; the NEAR leg occludes at overlap",
                    0.8,
                ),
                phase(
                    "Near up",
                    "Body rises over the NEAR toe; FAR leg reaches ahead for its contact",
                    0.9,
                ),
                phase(
                    "Far contact",
                    "FAR leg plants ahead under the hip; NEAR leg trails behind on the toe",
                    1.0,
                ),
                phase(
                    "Far down",
                    "Weight compresses over the FAR planted foot; NEAR heel lifts",
                    0.9,
                ),
                phase(
                    "Far passing",
                    "NEAR leg swings through beside the FAR planted leg; the FAR leg occludes at overlap",
                    0.8,
                ),
                phase(
                    "Far up",
                    "Body rises over the FAR toe; NEAR leg reaches ahead to close the loop",
                    0.9,
                ),
            ],
            true,
            true,
        );
    }
    if ["run", "running", "sprint"]
        .iter()
        .any(|word| contains_word(&lower, word))
    {
        return (
            "a looping run cycle",
            8,
            vec![
                phase(
                    "Near contact",
                    "NEAR leg reaches into contact under the hips; FAR leg trails behind with a bent knee",
                    0.8,
                ),
                phase(
                    "Near compression",
                    "NEAR stance leg absorbs and lowers the body; FAR leg folds and swings forward",
                    0.8,
                ),
                phase(
                    "Near drive",
                    "NEAR toe drives the body forward while the FAR knee drives through",
                    0.7,
                ),
                phase(
                    "Flight",
                    "Both feet airborne; FAR leg now reaching ahead while the NEAR leg trails",
                    0.7,
                ),
                phase(
                    "Far contact",
                    "FAR leg reaches into contact under the hips; NEAR leg trails behind with a bent knee",
                    0.8,
                ),
                phase(
                    "Far compression",
                    "FAR stance leg absorbs and lowers the body; NEAR leg folds and swings forward",
                    0.8,
                ),
                phase(
                    "Far drive",
                    "FAR toe drives the body forward while the NEAR knee drives through",
                    0.7,
                ),
                phase(
                    "Flight return",
                    "Both feet airborne again; NEAR leg reaches ahead to close the loop",
                    0.7,
                ),
            ],
            true,
            true,
        );
    }
    if ["idle", "breath", "breathing", "blink"]
        .iter()
        .any(|word| contains_word(&lower, word))
    {
        return (
            "a subtle looping idle",
            5,
            vec![
                phase("Rest", "Hold the grounded neutral silhouette", 1.2),
                phase("Inhale", "Lift the torso or secondary forms slightly", 1.0),
                phase("Apex", "Reach the smallest readable high point", 0.9),
                phase("Exhale", "Ease back through neutral", 1.0),
                phase("Settle", "Reconnect to the opening pose", 1.2),
            ],
            true,
            false,
        );
    }
    if ["open", "opening", "close", "closing", "unfold"]
        .iter()
        .any(|word| contains_word(&lower, word))
    {
        return (
            "a hinged or mechanical object action",
            8,
            vec![
                phase("Closed", "Establish the locked object body", 1.0),
                phase(
                    "Anticipation",
                    "Give the moving part a small preparatory shift",
                    0.8,
                ),
                phase(
                    "Early opening",
                    "Begin rotation around the fixed hinge",
                    0.8,
                ),
                phase("Opening", "Continue the connected mechanical arc", 0.8),
                phase("Reveal", "Reach the readable open state or payload", 1.1),
                phase("Hold", "Let the open result read at game scale", 1.2),
                phase("Closing", "Reverse through the same hinge path", 0.9),
                phase("Settle", "Return cleanly to the closed loop pose", 1.0),
            ],
            true,
            false,
        );
    }
    if ["attack", "slash", "strike", "swing", "shoot"]
        .iter()
        .any(|word| contains_word(&lower, word))
    {
        return (
            "a one-shot attack",
            7,
            vec![
                phase("Start", "Establish the neutral combat silhouette", 0.8),
                phase(
                    "Anticipation",
                    "Make the direction of force readable; weight shifts onto the FAR leg",
                    1.0,
                ),
                phase(
                    "Wind-up",
                    "Load the body or weapon over the grounded FAR leg",
                    1.0,
                ),
                phase(
                    "Acceleration",
                    "Move rapidly toward contact; NEAR leg starts its step",
                    0.7,
                ),
                phase(
                    "Impact",
                    "Decisive contact pose; NEAR foot planted, FAR heel lifted",
                    0.9,
                ),
                phase("Follow-through", "Carry momentum past impact", 0.9),
                phase("Recovery", "Return to a stable pose", 1.1),
            ],
            false,
            true,
        );
    }
    if ["explosion", "burst", "spark", "smoke", "effect", "vfx"]
        .iter()
        .any(|word| contains_word(&lower, word))
    {
        return (
            "a staged visual effect",
            8,
            vec![
                phase("Seed", "Establish the compact source", 0.5),
                phase("Ignition", "Create the first high-energy change", 0.6),
                phase("Expansion", "Grow the effect silhouette rapidly", 0.7),
                phase("Peak", "Reach maximum area and brightness", 0.8),
                phase(
                    "Breakup",
                    "Separate the main mass into readable fragments",
                    0.7,
                ),
                phase("Dissipation", "Reduce opacity and energy", 0.9),
                phase("Fade", "Leave only secondary traces", 1.0),
                phase(
                    "Clear",
                    "Return to transparent for a clean loop or finish",
                    0.6,
                ),
            ],
            false,
            false,
        );
    }
    (
        "a general readable motion",
        6,
        vec![
            phase("Start", "Establish the initial silhouette", 1.0),
            phase("Anticipation", "Prepare the primary change", 0.9),
            phase("Action", "Perform the main movement", 0.8),
            phase("Peak", "Show the clearest extreme pose", 1.0),
            phase("Recovery", "Resolve the movement", 0.9),
            phase("Settle", "Reach the final or looping pose", 1.0),
        ],
        lower.contains("loop"),
        false,
    )
}

pub(super) const fn phase(
    name: &'static str,
    description: &'static str,
    weight: f64,
) -> PhaseDefinition {
    PhaseDefinition {
        name,
        description,
        weight,
    }
}
