const Global = {
  init() {
    this.mainEl = document.getElementsByTagName('main')[0]; 
    this.templateMain = document.getElementById('templateMain');
    this.templateLoader = document.getElementById('templateLoader');
    this.templateProject = document.getElementById('templateProject');
    this.templatePage = document.getElementById('templatePage');
    return this;
  },
  assignFromTemplate: function(template) {
    const newContent = template.content.cloneNode(true);
    this.mainEl.innerHTML = '';
    while(newContent.firstChild) {
      this.mainEl.appendChild(newContent.removeChild(newContent.firstChild));
    }
  },
  loadFromRequest: async function(request) {
    try {
      const response = await request;
      if (response.status >= 300) {
        throw '';
      }

      const data = await response.json();
      this.project = data;
    } catch (e) {
      console.log(e);
      throw e;
    }

    const url = '/project/edit/' + this.project.identifier;
    window.history.pushState({}, 'Edit Project', url);

    this.assignFromTemplate(this.templateProject);
    this.setUpProjectPage();
  },
  setUpProjectPage: function() {
    const pageList = this.mainEl.querySelector('#pageList');
    if (!isNaN(this.selectedPageIdx) && this.project.pages.length > 0) {
      this.selectedPageIdx = Math.min(this.selectedPageIdx, this.project.pages.length - 1);
    } else {
      this.selectedPageIdx = undefined;
    }

    const videoImgReplacement = this.mainEl.querySelector('#outputVideoReplacement');
    const audio = this.mainEl.querySelector('#pageAudio');

    const audioSetter = this.mainEl.querySelector('#pageAudioSetter');
    const input = audioSetter.querySelector('input');
    const button = audioSetter.querySelector('button');


    const projectForHandler = this;

    button.onclick = async function() {
      if (input.files.length == 0) {
        // TODO: show error.
        console.log('no input files');
        return;
      }

      if (isNaN(projectForHandler.selectedPageIdx)) {
        console.log('no page selected');
        return;
      }

      const pageUrl = '/project/page/' + projectForHandler.selectedPageIdx;
      const selectedPageIdx = projectForHandler.selectedPageIdx;
      await projectForHandler.loadFromRequest(fetch(pageUrl, {
        method: 'put',
        body: input.files[0],
      }));
    };

    this.project.pages.forEach((el, idx) => {
      const listItem = this.templatePage.content.querySelector('.page-list-item').cloneNode(true);
      const preview = listItem.querySelector('.page-preview');
      const audioIndicator = listItem.querySelector('.page-audio-indicator');

      listItem.onclick = function() {
        projectForHandler.selectedPageIdx = idx;
        projectForHandler.updateSelectedPageState();
      };

      preview.querySelector('img').src = el.img_url;
      pageList.appendChild(listItem);
      if (el.audio_url) {
        audioIndicator.classList.add('page-audio-yes');
      } else {
        audioIndicator.classList.add('page-audio-no');
      }
    });

    const create = this.mainEl.querySelector('#createVideo');
    create.onclick = async function() {
      try {
        create.setAttribute('disabled', '');
        await projectForHandler.loadFromRequest(fetch('/project/render', { method: 'post' }));
      } finally {
        create.removeAttribute('disabled');
      }
    };

    if (!isNaN(this.selectedPageIdx)) {
      this.updateSelectedPageState();
    }

    const download = this.mainEl.querySelector('#downloadVideo');
    if (this.project.output) {
      const link = document.createElement('a');
      link.href = this.project.output;
      link.setAttribute('download', '');
      link.setAttribute('target', '_blank');
      link.setAttribute('type', 'video/mp4');

      download.removeAttribute('disabled');
      download.onclick = () => {
        console.log(link);
        link.click();
      };
    }
  },
  updateSelectedPageState: function() {
    const videoImgReplacement = this.mainEl.querySelector('#outputVideoReplacement');
    const audio = this.mainEl.querySelector('#pageAudio');

    const el = this.project.pages[this.selectedPageIdx];
    if (el.audio_url !== null && el.audio_url !== undefined) {
      audio.removeAttribute('disabled');
      audio.src = el.audio_url;
    } else {
      audio.setAttribute('disabled', '');
    }

    videoImgReplacement.src = el.img_url;
  },
  setUpIndexPage: function() {
    const fileDrop = this.mainEl.querySelector('#fileDrop');
    const fileInput = this.mainEl.querySelector('#fileInput');
    const createProject = this.mainEl.querySelector('#createProject');

    fileDrop.ondragover = fileDrop.ondragenter = (evt) => { evt.preventDefault(); }
    fileDrop.ondrop = (evt) => {
      fileInput.files = evt.dataTransfers.file;
    };

    const that = this;
    createProject.onclick = async function(evt) {
      if (fileInput.files.length < 0) {
        /* TODO: error */
        return;
      }

      const req = fetch('/project/new', {
        method: 'PUT',
        body: fileInput.files[0]
      });

      await that.loadFromRequest(req);
    };
  }
};

window.onload = function() {
  const global = Global.init();
  if (window.location.pathname == '/') {
    global.assignFromTemplate(global.templateMain);
    global.setUpIndexPage();
  } else {
    global.assignFromTemplate(global.templateLoader);
    global.loadFromRequest(fetch('/project/get')).catch((e) => {
      console.log(e);
      global.assignFromTemplate(global.templateMain);
      global.setUpIndexPage();
      console.log('Loading failed, going back to main page');
      window.history.pushState({}, '', '/')
    });
  }
};
