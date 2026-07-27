<script lang="ts">
  // Ícones REAIS das marcas (mesmos SVGs da v1), renderizados inline e tingidos
  // via currentColor — como a v1 fazia com template images.
  import claudeSvg from "../assets/icons/claude-ai.svg?raw";
  import openaiSvg from "../assets/icons/openai.svg?raw";
  import opencodeSvg from "../assets/icons/opencode.svg?raw";
  import shellSvg from "../assets/icons/shell.svg?raw";

  let { id, size = 16 }: { id: string; size?: number } = $props();

  const RAW: Record<string, string> = {
    claude: claudeSvg,
    codex: openaiSvg,
    opencode: opencodeSvg,
    shell: shellSvg,
  };

  // Tira width/height do root pra o SVG escalar pelo container.
  function normalize(svg: string): string {
    return svg
      .replace(/<\?xml[^>]*\?>/g, "")
      .replace(/<!--[\s\S]*?-->/g, "")
      .replace(/(<svg\b[^>]*?)\swidth=(['"])[^'"]*\2/i, "$1")
      .replace(/(<svg\b[^>]*?)\sheight=(['"])[^'"]*\2/i, "$1");
  }

  const html = $derived(normalize(RAW[id] ?? RAW.shell));
</script>

<span class="icon" style="width:{size}px;height:{size}px">{@html html}</span>

<style>
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    line-height: 0;
  }
  /* Monocromático na cor herdada (cor da marca), como na v1. */
  .icon :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
  .icon :global(svg path),
  .icon :global(svg rect),
  .icon :global(svg circle),
  .icon :global(svg polygon) {
    fill: currentColor !important;
    stroke: none;
  }
  /* Máscaras/clip do opencode precisam continuar brancas pra não sumir o desenho. */
  .icon :global(svg mask path),
  .icon :global(svg clipPath rect),
  .icon :global(svg defs *) {
    fill: #fff !important;
  }
</style>
