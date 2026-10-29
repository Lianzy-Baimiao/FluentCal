// Accessible names for form controls. Settings cards/rows and FField render a title with
// an id and provide it; controls inside inject it and point aria-labelledby at it, unless
// the caller passes aria-label / aria-labelledby or a `label` prop.
import { inject, provide, useAttrs, type InjectionKey } from 'vue';

export interface FieldContext {
  titleId: string;
  /** id of the description element, if one is rendered */
  descId: () => string | undefined;
  /** plain text of the title, for popups that cannot reference the id */
  titleText: () => string;
}

const KEY: InjectionKey<FieldContext | null> = Symbol('f-field');
let seq = 0;

export function uid(prefix = 'f'): string {
  return `${prefix}-${(++seq).toString(36)}`;
}

/** Called by the component that renders the visible label. */
export function provideField(titleText: () => string, hasDesc: () => boolean): { titleId: string; descId: string } {
  const titleId = uid('ft');
  const descId = uid('fd');
  provide(KEY, { titleId, descId: () => (hasDesc() ? descId : undefined), titleText });
  return { titleId, descId };
}

export function injectField(): FieldContext | null {
  return inject(KEY, null);
}

type Attrs = Record<string, unknown>;
const NAME_KEYS = ['aria-label', 'aria-labelledby', 'aria-describedby', 'ariaLabel', 'ariaLabelledby', 'ariaDescribedby'];

/** Resolves the accessible-name attributes of a control. Call the returned functions in
 *  the template (fallthrough attrs are not reactive in computed()). */
export function useFieldName(label: () => string | undefined, fallback?: () => string | undefined) {
  const attrs = useAttrs() as Attrs;
  const ctx = injectField();
  const own = () => (attrs['aria-label'] as string | undefined) ?? (attrs.ariaLabel as string | undefined);
  const ownBy = () => (attrs['aria-labelledby'] as string | undefined) ?? (attrs.ariaLabelledby as string | undefined);
  const ownDesc = () => (attrs['aria-describedby'] as string | undefined) ?? (attrs.ariaDescribedby as string | undefined);
  return {
    ctx,
    /** aria-label / aria-labelledby / aria-describedby for the focusable element */
    attrs(): Attrs {
      const describedby = ownDesc() ?? ctx?.descId();
      if (own()) return { 'aria-label': own(), 'aria-describedby': describedby };
      if (ownBy()) return { 'aria-labelledby': ownBy(), 'aria-describedby': describedby };
      const l = label();
      if (l) return { 'aria-label': l, 'aria-describedby': describedby };
      if (ctx) return { 'aria-labelledby': ctx.titleId, 'aria-describedby': describedby };
      return { 'aria-label': fallback?.() || undefined, 'aria-describedby': describedby };
    },
    /** plain-text name (for listboxes and other popups) */
    text(): string | undefined {
      return own() ?? label() ?? ctx?.titleText() ?? fallback?.();
    },
  };
}

/** Splits fallthrough attrs: class/style stay on the visual root, everything else
 *  (id, data-*, listeners, …) goes to the focusable element. Name attrs are excluded from
 *  `control()`; get them from useFieldName().attrs(). `forward()` keeps them, for wrappers
 *  that pass attrs on to another Fluent control. */
export function useSplitAttrs() {
  const attrs = useAttrs() as Attrs;
  const pick = (skip: string[]) => Object.fromEntries(Object.entries(attrs).filter(([k]) => !skip.includes(k)));
  return {
    root: (): Attrs => ({ class: attrs.class, style: attrs.style }),
    control: (): Attrs => pick(['class', 'style', ...NAME_KEYS]),
    forward: (): Attrs => pick(['class', 'style']),
  };
}
