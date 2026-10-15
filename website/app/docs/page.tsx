import Doc from './doc';
import { find } from './pages';

export default function DocsHome() {
  return <Doc page={find('introduction')!} />;
}
