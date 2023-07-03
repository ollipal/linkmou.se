import { Popover } from "@kobalte/core";
import { createSignal, JSXElement, onCleanup } from "solid-js";
//import { CrossIcon } from "some-icon-library";
import "./Pop.css";
function Pop({children} : {children: JSXElement}) {
  const [open, setOpen] = createSignal(false);

  let timeout : NodeJS.Timeout | undefined = undefined;

  onCleanup(() => {
    clearTimeout(timeout);
  })

  return (
    <Popover.Root open={open()} onOpenChange={(isOpen) => {
      if (!isOpen) {
        setOpen(false);
        clearTimeout(timeout);
        return;
      } else {
        setOpen(true);
        timeout = setTimeout(() => setOpen(false), 3000)
      }
    }}>
      <Popover.Trigger class="popover__trigger">{children}</Popover.Trigger>
      <Popover.Portal>
        <Popover.Content class="popover__content">
          <Popover.Arrow />
          <div class="popover__header">
            <Popover.Title class="popover__title">Copied!</Popover.Title>
            {/* <Popover.CloseButton class="popover__close-button">
              <CrossIcon />
            </Popover.CloseButton> */}
          </div>
          {/* <Popover.Description class="popover__description">
            A UI toolkit for building accessible web apps and design systems with SolidJS.
          </Popover.Description> */}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

export default Pop;