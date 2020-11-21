window.onload = (() => {
  const mainEl = document.getElementsByTagName('main')[0];

  const templateMain = document.getElementById('templateMain');
  const templateLoader = document.getElementById('templateLoader');
  const templateProject = document.getElementById('templateProject');
  const templatePage = document.getElementById('templatePage');

  this.assignFromTemplate = function(template) {
    const newContent = template.content.cloneNode(true);
    mainEl.innerHTML = '';
    while(newContent.firstChild) {
      mainEl.appendChild(newContent.removeChild(newContent.firstChild));
    }
  };

  this.loadFromRequest = async function(request) {
    this.assignFromTemplate(templateLoader);

    try {
      const response = await request;
      if (response.status >= 300) {
        throw '';
      }

      this.project = await response.json();
    } catch (_) {
      this.assignFromTemplate(templateMain);
      this.setUpIndexPage();
      console.log('Loading failed, going back to main page');
      window.history.pushState({}, '', '/')
      return;
    }

    const url = '/project/edit/' + this.project.identifier;
    window.history.pushState({}, 'Edit Project', url);

    this.assignFromTemplate(templateProject);
    this.setUpProjectPage();
  };

  this.setUpProjectPage = function() {
    const pageList = mainEl.querySelector('#pageList');
    this.project.pages.forEach((el, idx) => {
      const listItem = templatePage.content.querySelector('.page-list-item').cloneNode(true);
      const preview = listItem.querySelector('.page-preview');
      preview.querySelector('img').src = el.img_url;
      const audio = listItem.querySelector('audio');
      if (el.audio_url) {
        audio.src = el.audio_url;
      }

      // TODO: lazy generate and use thumbnails?
      const input = listItem.querySelector('input');
      const button = listItem.querySelector('button');

      button.onclick = async function() {
        if (input.files.len == 0) {
          // TODO: show error.
          return;
        }

        await this.loadFromRequest(fetch('/project/page/' + idx, {
          method: 'put',
          body: input.file,
        }));
      };

      pageList.appendChild(listItem);
    });
  };

  this.setUpIndexPage = function() {
    const fileDrop = mainEl.querySelector('#fileDrop');
    const fileInput = mainEl.querySelector('#fileInput');
    const createProject = mainEl.querySelector('#createProject');

    fileDrop.ondragover = fileDrop.ondragenter = (evt) => { evt.preventDefault(); }
    fileDrop.ondrop = (evt) => {
      fileInput.files = evt.dataTransfers.file;
    };

    createProject.onclick = async function(evt) {
      if (fileInput.files.len < 0) {
        /* TODO: error */
        return;
      }

      const req = fetch('/project/new', {
        method: 'PUT',
        body: fileInput.files[0]
      });

      await loadFromRequest(req);
    };
  };

  if (window.location.pathname == '/') {
    this.assignFromTemplate(templateMain);
    this.setUpIndexPage();
  } else {
    this.assignFromTemplate(templateLoader);
    this.loadFromRequest(fetch('/project/get'));
  }
});
