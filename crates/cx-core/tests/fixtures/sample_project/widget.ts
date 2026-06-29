// A small TypeScript widget module.

export interface Drawable {
  draw(): void;
}

export type Size = { width: number; height: number };

export class Button implements Drawable {
  constructor(private label: string) {}

  draw(): void {
    console.log(`[ ${this.label} ]`);
  }
}

export function makeButton(label: string): Button {
  return new Button(label);
}
