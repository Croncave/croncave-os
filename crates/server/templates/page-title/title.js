// Fetch a page and print its title. Change URL to any page.
const URL = process.env.URL || "http://127.0.0.1:8080/demo/page";
const fs = require("fs");

(async () => {
  const res = await fetch(URL);
  const html = await res.text();
  const title = (html.match(/<title>([^<]*)<\/title>/i) || [])[1] || "(no title)";
  console.log(`${URL} -> ${title}`);
  if (process.env.CRONCAVE_SUMMARY) {
    fs.writeFileSync(process.env.CRONCAVE_SUMMARY, JSON.stringify({ headline: `Title: ${title}`, values: { Status: res.status } }));
  }
})().catch((e) => {
  console.error(e.message);
  process.exit(1);
});
