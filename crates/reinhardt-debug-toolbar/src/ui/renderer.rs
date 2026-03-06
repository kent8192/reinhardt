//! Toolbar HTML rendering

use crate::error::ToolbarResult;
use crate::panels::PanelStats;

/// Render complete toolbar HTML
pub fn render_toolbar(panel_stats: &[PanelStats]) -> ToolbarResult<String> {
	let panel_handles = panel_stats
		.iter()
		.map(|p| {
			format!(
				r#"<div class="djdt-panel-handle" data-panel="{}" onclick="switchPanel('{}')">
					{}: {}
				</div>"#,
				p.panel_id, p.panel_id, p.panel_name, p.summary
			)
		})
		.collect::<Vec<_>>()
		.join("");

	let panel_contents = panel_stats
		.iter()
		.map(|p| {
			let rendered = p.rendered_html.as_ref().cloned().unwrap_or_else(|| {
				format!(
					"<pre>{}</pre>",
					serde_json::to_string_pretty(&p.data).unwrap_or_default()
				)
			});

			format!(
				r#"<div class="djdt-panel-content" id="djdt-panel-{}" style="display: none;">{}</div>"#,
				p.panel_id, rendered
			)
		})
		.collect::<Vec<_>>()
		.join("");

	Ok(format!(
		r#"
<div id="djDebug" class="djdt-hidden">
	<div class="djdt-toolbar">
		<div class="djdt-handle" onclick="toggleToolbar()">
			<span class="djdt-icon">&#9776;</span>
			<span class="djdt-title">Reinhardt Debug Toolbar</span>
		</div>
		<div class="djdt-panels">
			{}
		</div>
	</div>
	<div class="djdt-content">
		{}
	</div>
</div>
<style>
{}
</style>
<script>
{}
</script>
		"#,
		panel_handles, panel_contents, TOOLBAR_CSS, TOOLBAR_JS
	))
}

const TOOLBAR_CSS: &str = r#"
#djDebug {
	position: fixed;
	top: 0;
	right: 0;
	width: 400px;
	height: 100vh;
	background: #f8f9fa;
	box-shadow: -2px 0 5px rgba(0,0,0,0.1);
	overflow-y: auto;
	z-index: 999999;
	font-family: 'Monaco', 'Menlo', monospace;
	font-size: 12px;
	transition: transform 0.3s ease;
}

#djDebug.djdt-hidden {
	transform: translateX(380px);
}

.djdt-toolbar {
	background: #343a40;
	color: white;
	padding: 10px;
	position: sticky;
	top: 0;
	z-index: 1;
}

.djdt-handle {
	cursor: pointer;
	padding: 5px;
	user-select: none;
}

.djdt-panel-handle {
	display: inline-block;
	padding: 5px 10px;
	margin: 2px;
	background: #495057;
	cursor: pointer;
	border-radius: 3px;
	transition: background 0.2s;
}

.djdt-panel-handle:hover {
	background: #6c757d;
}

.djdt-panel-handle.active {
	background: #007bff;
}

.djdt-panel-content {
	padding: 15px;
}

.djdt-table {
	width: 100%;
	border-collapse: collapse;
	margin: 10px 0;
}

.djdt-table th,
.djdt-table td {
	padding: 8px;
	border: 1px solid #dee2e6;
	text-align: left;
}

.djdt-table th {
	background: #e9ecef;
	font-weight: bold;
}
"#;

const TOOLBAR_JS: &str = r#"
function toggleToolbar() {
	const toolbar = document.getElementById('djDebug');
	toolbar.classList.toggle('djdt-hidden');
	localStorage.setItem('djdt-collapsed', toolbar.classList.contains('djdt-hidden'));
}

function switchPanel(panelId) {
	document.querySelectorAll('.djdt-panel-content').forEach(panel => {
		panel.style.display = 'none';
	});

	document.querySelectorAll('.djdt-panel-handle').forEach(handle => {
		handle.classList.remove('active');
	});

	const panel = document.getElementById('djdt-panel-' + panelId);
	if (panel) {
		panel.style.display = 'block';
	}

	const handle = document.querySelector('[data-panel="' + panelId + '"]');
	if (handle) {
		handle.classList.add('active');
	}

	localStorage.setItem('djdt-active-panel', panelId);
}

window.addEventListener('DOMContentLoaded', () => {
	const collapsed = localStorage.getItem('djdt-collapsed') === 'true';
	if (collapsed) {
		document.getElementById('djDebug').classList.add('djdt-hidden');
	}

	const activePanel = localStorage.getItem('djdt-active-panel');
	if (activePanel) {
		switchPanel(activePanel);
	} else {
		const firstHandle = document.querySelector('.djdt-panel-handle');
		if (firstHandle) {
			switchPanel(firstHandle.dataset.panel);
		}
	}
});
"#;
