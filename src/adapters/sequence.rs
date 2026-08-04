//! Sequence rendering with semantic fallbacks for valid models rejected upstream.

use std::collections::{BTreeSet, HashMap};

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxEdge, BoxGroup, BoxLayout, BoxNode, EdgeLegend, EdgeMarker, EdgeStyle,
};
use crate::adapters::{nonempty_or, to_ascii_options};
use crate::ansi::{sanitize_label_text, strip_ansi};
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;
use merman_ascii::AsciiError;
use merman_core::diagrams::sequence::{
    SequenceActor, SequenceDiagramRenderModel, SequenceMessage, SequenceMessagePayload,
};

const MAX_SEQUENCE_ITEMS: usize = 8_192;

pub fn render_sequence(
    model: &SequenceDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    ensure_sequence_limit(model)?;
    if model.actor_order.is_empty()
        && model.actors.is_empty()
        && model.messages.is_empty()
        && model.notes.is_empty()
        && model.boxes.is_empty()
    {
        return Ok(empty_sequence(model));
    }

    let geometry_model = sequence_geometry_model(model);
    if model
        .actors
        .values()
        .any(|actor| !actor.properties.is_empty())
    {
        match merman_ascii::render_sequence(&geometry_model, &to_ascii_options(opts)) {
            Ok(output) => {
                let output = append_actor_properties(output, model, opts.max_width);
                if preview_fits(&output, opts) {
                    return Ok(output);
                }
                return render_semantic_fallback(model, opts);
            }
            Err(AsciiError::UnsupportedFeature { .. } | AsciiError::RenderLimitExceeded { .. }) => {
                return render_semantic_fallback(model, opts);
            }
            Err(error) => return Err(error.into()),
        }
    }

    match merman_ascii::render_sequence(&geometry_model, &to_ascii_options(opts)) {
        Ok(output) if !output.trim().is_empty() && preview_fits(&output, opts) => Ok(output),
        Ok(_) => render_semantic_fallback(model, opts),
        Err(AsciiError::UnsupportedFeature { .. } | AsciiError::RenderLimitExceeded { .. }) => {
            render_semantic_fallback(model, opts)
        }
        Err(error) => Err(error.into()),
    }
}

fn sequence_geometry_model(model: &SequenceDiagramRenderModel) -> SequenceDiagramRenderModel {
    let mut geometry = model.clone();
    geometry.title = geometry.title.as_deref().map(sanitize_label_text);
    geometry.acc_title = geometry.acc_title.as_deref().map(sanitize_label_text);
    for actor in geometry.actors.values_mut() {
        actor.name = sanitize_label_text(&actor.name);
        actor.description = sanitize_label_text(&actor.description);
        actor.properties.clear();
    }
    for sequence_box in &mut geometry.boxes {
        sequence_box.name = sequence_box.name.as_deref().map(sanitize_label_text);
        sequence_box.wrap = true;
    }
    for message in &mut geometry.messages {
        if let SequenceMessagePayload::Text(text) = &mut message.message {
            *text = sanitize_label_text(text);
        }
        // Unwrapped labels can overwrite a neighboring lifeline even when the final row still
        // fits the viewport. Wrapping is a presentation-only adaptation; the canonical model
        // retains the parsed Mermaid flag.
        message.wrap = true;
    }
    for note in &mut geometry.notes {
        note.message = sanitize_label_text(&note.message);
        note.wrap = true;
    }
    geometry
}

fn ensure_sequence_limit(model: &SequenceDiagramRenderModel) -> Result<()> {
    let requested = model
        .actors
        .values()
        .fold(model.actors.len(), |count, actor| {
            count
                .saturating_add(actor.links.len())
                .saturating_add(actor.properties.len())
        })
        .saturating_add(model.boxes.len())
        .saturating_add(
            model
                .boxes
                .iter()
                .map(|sequence_box| sequence_box.actor_keys.len())
                .fold(0usize, usize::saturating_add),
        )
        .saturating_add(model.messages.len())
        .saturating_add(model.notes.len());
    if requested > MAX_SEQUENCE_ITEMS {
        return Err(MermansiError::RenderLimit {
            context: "sequence items",
            requested,
            limit: MAX_SEQUENCE_ITEMS,
        });
    }
    Ok(())
}

fn preview_fits(output: &str, opts: &MermansiOptions) -> bool {
    let mut rows = 0usize;
    for line in output.lines() {
        rows = rows.saturating_add(1);
        if rows > opts.max_height || crate::str_display_width(&strip_ansi(line)) > opts.max_width {
            return false;
        }
    }
    true
}

fn render_semantic_fallback(
    model: &SequenceDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let actor_ids = ordered_actor_ids(model);
    let actor_parents = sequence_box_parents(model);
    let groups = model
        .boxes
        .iter()
        .enumerate()
        .map(|(order, sequence_box)| BoxGroup {
            id: sequence_box_id(order),
            lines: vec![
                sequence_box
                    .name
                    .as_deref()
                    .map(sanitize_label_text)
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| "participant group".to_owned()),
            ],
            parent: None,
            columns: None,
            span: 1,
            order,
        })
        .collect::<Vec<_>>();
    let nodes = actor_ids
        .iter()
        .enumerate()
        .map(|(order, id)| BoxNode {
            id: id.clone(),
            lines: actor_lines(model, id),
            dividers: Vec::new(),
            parent: actor_parents.get(id).cloned(),
            span: 1,
            order: model.boxes.len().saturating_add(order),
        })
        .collect::<Vec<_>>();
    let known = actor_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let edges = model
        .messages
        .iter()
        .filter_map(|message| message_edge(message, &known))
        .collect::<Vec<_>>();

    let mut output = if nodes.is_empty() && groups.is_empty() {
        empty_sequence(model)
    } else {
        box_geometry::render(
            &BoxDiagram {
                family: "sequence",
                title: model.title.clone().or_else(|| model.acc_title.clone()),
                nodes,
                groups,
                spacers: Vec::new(),
                edges,
                columns: None,
                layout: BoxLayout::Packed,
                edge_legend: EdgeLegend::None,
            },
            opts,
        )?
    };
    append_events(&mut output, model, opts.max_width);
    Ok(output)
}

fn ordered_actor_ids(model: &SequenceDiagramRenderModel) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut ids = Vec::new();
    for id in &model.actor_order {
        if seen.insert(id.clone()) {
            ids.push(id.clone());
        }
    }
    for id in model.actors.keys() {
        if seen.insert(id.clone()) {
            ids.push(id.clone());
        }
    }
    for message in &model.messages {
        for id in [&message.from, &message.to].into_iter().flatten() {
            if seen.insert(id.clone()) {
                ids.push(id.clone());
            }
        }
    }
    ids
}

fn sequence_box_parents(model: &SequenceDiagramRenderModel) -> HashMap<String, String> {
    let mut parents = HashMap::new();
    for (index, sequence_box) in model.boxes.iter().enumerate() {
        for actor in &sequence_box.actor_keys {
            parents
                .entry(actor.clone())
                .or_insert_with(|| sequence_box_id(index));
        }
    }
    parents
}

fn sequence_box_id(index: usize) -> String {
    format!("__sequence_box_{index}")
}

fn actor_lines(model: &SequenceDiagramRenderModel, id: &str) -> Vec<String> {
    let Some(actor) = model.actors.get(id) else {
        return vec![format!("{} (implicit)", sanitize_label_text(id))];
    };
    let mut lines = vec![actor_label(actor, id)];
    if !actor.actor_type.trim().is_empty() && actor.actor_type != "participant" {
        lines.push(format!("type: {}", sanitize_label_text(&actor.actor_type)));
    }
    if let Some(created) = model.created_actors.get(id) {
        lines.push(format!("created at event {created}"));
    }
    if let Some(destroyed) = model.destroyed_actors.get(id) {
        lines.push(format!("destroyed at event {destroyed}"));
    }
    for (key, value) in &actor.properties {
        lines.push(format!(
            "{}={}",
            sanitize_label_text(key),
            sanitize_label_text(&compact_json(value))
        ));
    }
    lines
}

fn actor_label(actor: &SequenceActor, id: &str) -> String {
    nonempty_or(
        sanitize_label_text(&actor.description).trim(),
        &sanitize_label_text(id),
    )
}

fn message_edge(message: &SequenceMessage, known: &BTreeSet<&str>) -> Option<BoxEdge> {
    let from = message.from.as_deref()?;
    let to = message.to.as_deref()?;
    if !known.contains(from) || !known.contains(to) {
        return None;
    }
    let dotted = matches!(message.message_type, 1 | 4 | 6);
    let marker = if matches!(message.message_type, 3 | 4) {
        EdgeMarker::Cross
    } else {
        EdgeMarker::Arrow
    };
    Some(BoxEdge {
        from: from.to_owned(),
        to: to.to_owned(),
        label: sanitize_label_text(message.message_text()),
        marker_start: EdgeMarker::None,
        marker_end: marker,
        style: if dotted {
            EdgeStyle::Dotted
        } else {
            EdgeStyle::Solid
        },
        from_side: None,
        to_side: None,
    })
}

fn append_events(output: &mut String, model: &SequenceDiagramRenderModel, max_width: usize) {
    if model.messages.is_empty() && model.notes.is_empty() {
        return;
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str("\nEvents\n");
    for (index, message) in model.messages.iter().enumerate() {
        let from = message
            .from
            .as_deref()
            .map(sanitize_label_text)
            .unwrap_or_else(|| "[control]".to_owned());
        let to = message
            .to
            .as_deref()
            .map(sanitize_label_text)
            .unwrap_or_else(|| "[control]".to_owned());
        let payload = match &message.message {
            SequenceMessagePayload::Text(text) => sanitize_label_text(text),
            SequenceMessagePayload::Autonumber(value) => compact_json(value),
        };
        let mut details = vec![format!(
            "{}. {} {} -> {}",
            index + 1,
            event_type(message.message_type),
            from,
            to
        )];
        if !payload.trim().is_empty() {
            details.push(payload);
        }
        if let Some(placement) = message.placement {
            details.push(format!("placement={placement}"));
        }
        if message.central_connection != 0 {
            details.push(format!("central={}", message.central_connection));
        }
        if message.activate {
            details.push("activate".to_owned());
        }
        append_wrapped_detail(output, &details.join(": "), max_width);
    }
    for note in &model.notes {
        let actor = sanitize_label_text(&compact_json(&note.actor));
        let text = format!(
            "note {actor} (placement={}): {}",
            note.placement,
            sanitize_label_text(&note.message)
        );
        append_wrapped_detail(output, &text, max_width);
    }
}

fn append_wrapped_detail(output: &mut String, text: &str, max_width: usize) {
    for line in box_geometry::wrap_words(text, max_width.saturating_sub(2)) {
        output.push_str("  ");
        output.push_str(&line);
        output.push('\n');
    }
}

fn event_type(message_type: i32) -> String {
    match message_type {
        0 => "solid message".to_owned(),
        1 => "dotted message".to_owned(),
        2 => "note".to_owned(),
        3 => "solid cross message".to_owned(),
        4 => "dotted cross message".to_owned(),
        5 => "solid open message".to_owned(),
        6 => "dotted open message".to_owned(),
        10 => "loop start".to_owned(),
        11 => "loop end".to_owned(),
        12 => "alt start".to_owned(),
        13 => "alt else".to_owned(),
        14 => "alt end".to_owned(),
        15 => "opt start".to_owned(),
        16 => "opt end".to_owned(),
        17 => "activation start".to_owned(),
        18 => "activation end".to_owned(),
        19 | 32 => "parallel start".to_owned(),
        20 => "parallel branch".to_owned(),
        21 => "parallel end".to_owned(),
        22 => "rect start".to_owned(),
        23 => "rect end".to_owned(),
        26 => "autonumber".to_owned(),
        27 => "critical start".to_owned(),
        28 => "critical option".to_owned(),
        29 => "critical end".to_owned(),
        30 => "break start".to_owned(),
        31 => "break end".to_owned(),
        other => format!("message type {other}"),
    }
}

fn compact_json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
}

fn empty_sequence(model: &SequenceDiagramRenderModel) -> String {
    let mut output = String::new();
    if let Some(title) = model
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
    {
        output.push_str(&sanitize_label_text(title));
        output.push_str("\n\n");
    }
    output.push_str("(empty sequence diagram)\n");
    output
}

fn append_actor_properties(
    mut output: String,
    model: &SequenceDiagramRenderModel,
    max_width: usize,
) -> String {
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str("\nActor properties\n");

    for actor_id in &model.actor_order {
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        if actor.properties.is_empty() {
            continue;
        }
        let actor_label = actor_label(actor, actor_id);
        for (key, value) in &actor.properties {
            let line = format!(
                "{actor_label}.{} = {}",
                sanitize_label_text(key),
                sanitize_label_text(&compact_json(value))
            );
            append_wrapped_detail(&mut output, &line, max_width);
        }
    }
    output
}
