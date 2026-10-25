// Chooser pages only: send German browsers to the German page, everyone else
// to the English one. The target comes from the html element (data-de, data-en),
// the root page falls back to the relative folders.
const root = document.documentElement.dataset;
const preferred = (navigator.languages && navigator.languages[0]) || navigator.language || 'en';
const german = preferred.toLowerCase().startsWith('de');
location.replace(german ? root.de || 'de/' : root.en || 'en/');
