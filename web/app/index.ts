import { Psx } from './src/psx'
import './styles/app.scss'
import './styles/controller-modal.scss'
import './styles/save-states-modal.scss'
import './styles/audio-modal.scss'
import './styles/cloud-saves-modal.scss'

const psx = new Psx()

psx.checkOauth()
