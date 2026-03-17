export function readTextFile(path: string): string {
  return Deno.readTextFileSync(path);
}
