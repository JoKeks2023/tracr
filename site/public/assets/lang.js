// Root page only: send German browsers to /de/, everyone else to /en/.
const preferred = (navigator.languages && navigator.languages[0]) || navigator.language || 'en';
location.replace(preferred.toLowerCase().startsWith('de') ? 'de/' : 'en/');
