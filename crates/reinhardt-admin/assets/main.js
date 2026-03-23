// Reinhardt Admin Panel - Bootstrap Script
//
// This placeholder provides a basic admin UI shell using UnoCSS
// utility classes. When the full WASM SPA is built (from src/pages/),
// this file will be replaced by the compiled WASM entry point.

(function() {
	"use strict";

	var app = document.getElementById("app");
	if (!app) return;

	app.className = "min-h-screen flex flex-col bg-slate-50";

	// Build header
	var header = document.createElement("header");
	header.className = "bg-blue-600 text-white px-6 py-4 flex items-center gap-4";
	var h1 = document.createElement("h1");
	h1.className = "text-xl font-semibold";
	h1.textContent = "Reinhardt Admin";
	header.appendChild(h1);

	// Build main content
	var main = document.createElement("main");
	main.className = "flex-1 p-6 max-w-5xl mx-auto w-full";
	var card = document.createElement("div");
	card.className = "bg-white border border-slate-200 rounded-lg p-6 shadow-sm";
	var h2 = document.createElement("h2");
	h2.className = "text-2xl font-bold text-slate-800";
	h2.textContent = "Admin Panel";
	card.appendChild(h2);
	var p = document.createElement("p");
	p.className = "mt-2 text-sm text-slate-500";
	p.textContent = "The admin panel is loading. If this message persists, " +
		"the WASM frontend may not be built yet. Run the admin SPA build " +
		"to enable full functionality.";
	card.appendChild(p);
	main.appendChild(card);

	// Mount
	app.appendChild(header);
	app.appendChild(main);
})();
